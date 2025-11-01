//! The `in_place` library provides an `InPlace` type for reading & writing a
//! file "in-place": data that you write ends up at the same filepath that you
//! read from, and `in_place` takes care of all the necessary mucking about
//! with temporary files for you.
//!
//! For example, given the file `somefile.txt`:
//!
//! ```text
//! 'Twas brillig, and the slithy toves
//!     Did gyre and gimble in the wabe;
//! All mimsy were the borogoves,
//!     And the mome raths outgrabe.
//! ```
//!
//! and the following program:
//!
//! ```no_run
//! use in_place::InPlace;
//! use std::io::{BufRead, BufReader, Write};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let inp = InPlace::new("somefile.txt").open()?;
//!     let reader = BufReader::new(inp.reader());
//!     let mut writer = inp.writer();
//!     for line in reader.lines() {
//!         let mut line = line?;
//!         line.retain(|ch| !"AEIOUaeiou".contains(ch));
//!         writeln!(writer, "{line}")?;
//!     }
//!     inp.save()?;
//!     Ok(())
//! }
//! ```
//!
//! after running the program, `somefile.txt` will have been edited in place,
//! reducing it to just:
//!
//! ```text
//! 'Tws brllg, nd th slthy tvs
//!     Dd gyr nd gmbl n th wb;
//! ll mmsy wr th brgvs,
//!     nd th mm rths tgrb.
//! ```
//!
//! and no sign of those pesky vowels remains!  If you want a sign of those
//! pesky vowels to remain, you can instead save the file's original contents
//! in, say, `somefile.txt~` by opening the file with:
//!
//! ```compile_fail
//! let inp = InPlace::new("somefile.txt")
//!     .backup(in_place::Backup::Append("~".into()))
//!     .open()?;
//! ```
//!
//! or save to `someotherfile.txt` with:
//!
//! ```compile_fail
//! let inp = InPlace::new("somefile.txt")
//!     .backup(in_place::Backup::Path("someotherfile.txt".into()))
//!     .open()?;
//! ```
//!
//! If you decide halfway through that you don't want to edit the file (say,
//! because an unrecoverable error occurs), calling `inp.discard()` instead of
//! `inp.save()` will close the file handles and reset things to the way they
//! were before.  Any changes are also discarded if `inp` is dropped without
//! saving, except that in that case any errors are silently ignored.

use std::error;
use std::ffi::OsString;
use std::fmt;
use std::fs::{File, metadata, rename, symlink_metadata};
use std::io;
use std::path::{Path, PathBuf};
use tempfile::{Builder, NamedTempFile, PersistError};

/// A builder for opening & editing a file in-place.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InPlace {
    path: PathBuf,
    backup: Option<Backup>,
    follow_symlinks: bool,
}

impl InPlace {
    /// Create a new `InPlace` instance for editing the given path (hereafter
    /// called the "edited path") in-place.
    pub fn new<P: AsRef<Path>>(path: P) -> InPlace {
        InPlace {
            path: path.as_ref().into(),
            backup: None,
            follow_symlinks: true,
        }
    }

    /// Move the edited file to the path given by `backup` when
    /// [`InPlaceFile::save()`] is called.
    ///
    /// Note that `in_place` does not create any parent directories of the
    /// backup path; it is the user's responsibility to ensure that the backup
    /// location is somewhere that a file can be moved to.
    ///
    /// If the backup path is the same as the edited path, the net effect will
    /// be as though no backup was configured.
    pub fn backup(&mut self, backup: Backup) -> &mut Self {
        self.backup = Some(backup);
        self
    }

    /// Do not move the edited file to a backup path.  This is the default
    /// behavior.
    ///
    /// This overrides any previous calls to [`InPlace::backup()`].
    pub fn no_backup(&mut self) -> &mut Self {
        self.backup = None;
        self
    }

    /// If `flag` is true (the default), the edited file path will be
    /// canonicalized, resolving any symlinks, before opening.  As a result, if
    /// the edited path points to a symlink, the file that the symlink points
    /// to will be the one edited (and moved to a backup location if so
    /// configured).
    ///
    /// If `flag` is false, the edited file path will not be canonicalized,
    /// though the file that it points to will still be edited through the
    /// symlink.  If a backup is configured, the symlink itself will be moved
    /// to the backup location.
    ///
    /// Note that this option only applies to the edited path; any symlinks in
    /// the backup path are not resolved.  As a result, if a backup path points
    /// to a symlink, backing up will obliterate the symlink (but not the file
    /// it points to) and replace it with the unmodified edited file.
    pub fn follow_symlinks(&mut self, flag: bool) -> &mut Self {
        self.follow_symlinks = flag;
        self
    }

    /// Open the edited path for reading and create a temporary file for
    /// writing.
    ///
    /// The exact set & order of operations may change in a future version, but
    /// currently it is as follows:
    ///
    /// - If `follow_symlinks` is true, the edited path is canonicalized.
    ///   Otherwise, if it is relative, the current directory is prepended.
    ///   (This ensures that changing the current directory while the
    ///   [`InPlaceFile`] is open will not mess anything up.)
    ///
    /// - If a backup is set, determine the backup path based on the
    ///   canonicalized/absolutized edited path.  If the result is a relative
    ///   path, the current directory is prepended.
    ///
    /// - Create a named temporary file in the edited path's parent directory.
    ///
    /// - If the edited path is not a symlink, copy its permission bits to the
    ///   temporary file.
    ///
    /// - Open the edited path for reading.
    ///
    /// # Errors
    ///
    /// See the documentation for the variants of [`InPlaceErrorKind`] for the
    /// operations & checks that this method can fail on.
    pub fn open(&self) -> Result<InPlaceFile, InPlaceError> {
        let path = if self.follow_symlinks {
            self.path
                .canonicalize()
                .map_err(InPlaceError::canonicalize)?
        } else {
            absolutize(&self.path)?
        };
        // Don't try to canonicalize backup_path, as it likely won't exist,
        // which would lead to an error
        let backup_path = match self.backup.as_ref() {
            Some(bkp) => Some(absolutize(&bkp.apply(&path)?)?),
            None => None,
        };
        let writer = mktemp(&path)?;
        copystats(&path, writer.as_file(), self.follow_symlinks)?;
        let reader = File::open(&path).map_err(InPlaceError::open)?;
        Ok(InPlaceFile {
            reader,
            writer,
            path,
            backup_path,
        })
    }
}

/// A path or path computation specifying where to back up an edited file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Backup {
    /// An explicit path at which to back up the edited file
    Path(PathBuf),
    /// Determine the path at which to backup the edited file by changing the
    /// file's filename to the given value
    FileName(OsString),
    /// Determine the path at which to backup the edited file by changing the
    /// file's extension (using [`Path::with_extension()`]) to the given value.
    /// Note that the value should generally not start with a period.
    Extension(OsString),
    /// Determine the path at which to backup the edited file by appending the
    /// given value to the filename
    Append(OsString),
}

impl Backup {
    fn apply(&self, path: &Path) -> Result<PathBuf, InPlaceError> {
        match self {
            Backup::Path(p) => {
                if p == Path::new("") {
                    Err(InPlaceError::empty_backup())
                } else {
                    Ok(p.clone())
                }
            }
            Backup::FileName(fname) => {
                if fname.is_empty() {
                    Err(InPlaceError::empty_backup())
                } else {
                    Ok(path.with_file_name(fname))
                }
            }
            Backup::Extension(ext) => Ok(path.with_extension(ext)),
            Backup::Append(ext) => {
                if ext.is_empty() {
                    Err(InPlaceError::empty_backup())
                } else {
                    match path.file_name() {
                        Some(fname) => {
                            let mut fname = fname.to_os_string();
                            fname.push(ext);
                            Ok(path.with_file_name(&fname))
                        }
                        None => Err(InPlaceError::no_filename()),
                    }
                }
            }
        }
    }
}

/// A file that is currently being edited in-place.
///
/// An `InPlaceFile` instance can be obtained via [`InPlace::open()`].
///
/// An `InPlaceFile` provides two file handles, one for reading the contents of
/// the edited file and for writing its new contents.  In order to update the
/// edited file with the written bytes, [`InPlaceFile::save()`] must be called
/// once writing is complete.  Alternatively, calling
/// [`InPlaceFile::discard()`] will discard all written bytes and leave the
/// edited file unmodified.
///
/// Dropping an `InPlaceFile` without calling `save()` has the same effect as
/// calling `discard()`, except that any errors are ignored.
#[derive(Debug)]
pub struct InPlaceFile {
    reader: File,
    writer: NamedTempFile,
    path: PathBuf,
    backup_path: Option<PathBuf>,
}

impl InPlaceFile {
    /// The reader file handle
    pub fn reader(&self) -> &File {
        &self.reader
    }

    /// The writer file handle
    pub fn writer(&self) -> &File {
        self.writer.as_file()
    }

    /// The path to the edited file.  If `follow_symlinks` was set to `true`,
    /// this will be a canonical path; otherwise, the path is only guaranteed
    /// to be absolute.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The path, if any, where the edited file will be backed up once
    /// [`InPlaceFile::save()`] is called.  This is an absolute path.
    pub fn backup_path(&self) -> Option<&Path> {
        self.backup_path.as_deref()
    }

    /// Save the unmodified edited file at the backup path, if any, and replace
    /// the edited file with the temporary output file.
    ///
    /// The exact set & order of operations may change in a future version, but
    /// currently it is as follows:
    ///
    /// - The file handle for the edited file is closed.
    ///
    /// - If a backup path is set, move the edited file to that location.
    ///
    /// - Persist the temporary file at the edited file's original location.
    ///   If this fails, and a backup path is set, try to move the backup back
    ///   to the original location, ignoring any errors.
    ///
    /// # Errors
    ///
    /// See the documentation for the variants of [`InPlaceErrorKind`] for the
    /// operations that this method can fail on.
    pub fn save(self) -> Result<(), InPlaceError> {
        drop(self.reader);
        if let Some(bp) = self.backup_path.as_ref() {
            rename(&self.path, bp).map_err(InPlaceError::save_backup)?;
        }
        match self.writer.persist(&self.path) {
            Ok(_) => Ok(()),
            Err(e) => {
                if let Some(bp) = self.backup_path.as_ref() {
                    let _ = rename(bp, &self.path);
                }
                Err(InPlaceError::persist(e))
            }
        }
    }

    /// Close all filehandles and do not update or back up the edited file.
    ///
    /// # Errors
    ///
    /// See the documentation for the variants of [`InPlaceErrorKind`] for the
    /// operations that this method can fail on.
    pub fn discard(self) -> Result<(), InPlaceError> {
        self.writer.close().map_err(InPlaceError::rmtemp)
    }
}

/// An error that can occur while opening, saving, or discarding an
/// [`InPlaceFile`].
///
/// Some errors are caused by failed I/O operations, while others are responses
/// to invalid paths or backup specifiers.  Only the first kind have source
/// errors, available via [`InPlaceError::as_io_error()`] and
/// [`InPlaceError::into_io_error()`] in addition to
/// [`std::error::Error::source()`].
#[derive(Debug)]
pub struct InPlaceError {
    kind: InPlaceErrorKind,
    source: Option<io::Error>,
}

impl InPlaceError {
    /// Returns an enum value describing the operation or check that failed
    pub fn kind(&self) -> InPlaceErrorKind {
        self.kind
    }

    /// Returns the [`std::io::Error`] that occurred, if any.  See the
    /// documentation of [`InPlaceErrorKind`] to find out which error kinds
    /// have source errors.
    pub fn as_io_error(&self) -> Option<&io::Error> {
        self.source.as_ref()
    }

    /// Consumes the [`InPlaceError`] and returns the inner [`std::io::Error`],
    /// if any.
    pub fn into_io_error(self) -> Option<io::Error> {
        self.source
    }

    fn get_metadata(source: io::Error) -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::GetMetadata,
            source: Some(source),
        }
    }

    fn set_metadata(source: io::Error) -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::SetMetadata,
            source: Some(source),
        }
    }

    fn no_parent() -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::NoParent,
            source: None,
        }
    }

    fn mktemp(source: io::Error) -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::Mktemp,
            source: Some(source),
        }
    }

    fn canonicalize(source: io::Error) -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::Canonicalize,
            source: Some(source),
        }
    }

    fn cwd(source: io::Error) -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::CurrentDir,
            source: Some(source),
        }
    }

    fn open(source: io::Error) -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::Open,
            source: Some(source),
        }
    }

    fn empty_backup() -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::EmptyBackup,
            source: None,
        }
    }

    fn no_filename() -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::NoFilename,
            source: None,
        }
    }

    fn save_backup(source: io::Error) -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::SaveBackup,
            source: Some(source),
        }
    }

    fn persist(source: PersistError) -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::PersistTemp,
            source: Some(source.error),
        }
    }

    fn rmtemp(source: io::Error) -> InPlaceError {
        InPlaceError {
            kind: InPlaceErrorKind::Rmtemp,
            source: Some(source),
        }
    }
}

impl fmt::Display for InPlaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind.message())
    }
}

impl error::Error for InPlaceError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.source.as_ref().map(|e| {
            let e2: &dyn error::Error = e;
            e2
        })
    }
}

/// An enumeration of the operations & checks that can fail while opening,
/// saving, or discarding an [`InPlaceFile`].
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum InPlaceErrorKind {
    /// Returned by [`InPlace::open()`] if attempting to canonicalize the
    /// edited path failed.
    ///
    /// This error kind occurs when the edited path does not exist and
    /// `follow_symlinks` is true.
    Canonicalize,

    /// Returned by [`InPlace::open()`] if attempting to fetch the current
    /// directory failed
    CurrentDir,

    /// Returned by [`InPlace::open()`] if the value within a [`Backup::Path`],
    /// [`Backup::FileName`], or [`Backup::Append`] backup specifier was empty.
    ///
    /// This error kind does not have a source error.
    EmptyBackup,

    /// Returned by [`InPlace::open()`] if attempting to fetch metadata &
    /// permission details about the edited file failed.
    ///
    /// This error kind occurs when the edited path does not exist and
    /// `follow_symlinks` is false.
    GetMetadata,

    /// Returned by [`InPlace::open()`] if attempting to create the temporary
    /// file failed
    Mktemp,

    /// Returned by [`InPlace::open()`] if a [`Backup::Append`] specifier was
    /// given and [`Path::file_name`] returned `None` for the edited path.
    ///
    /// This error kind does not have a source error.
    NoFilename,

    /// Returned by [`InPlace::open()`] if [`Path::parent`] returned `None` for
    /// the edited path (after canonicalization or absolutization).
    ///
    /// This error kind does not have a source error.
    NoParent,

    /// Returned by [`InPlace::open()`] if attempting to open the edited file
    /// for reading failed
    Open,

    /// Returned by [`InPlace::open()`] if attempting to copy the edited file's
    /// permissions to the temporary file failed
    SetMetadata,

    /// Returned by [`InPlaceFile::save()`] if attempting to persist the
    /// temporary file at the edited path failed
    PersistTemp,

    /// Returned by [`InPlaceFile::save()`] if attempting to move the edited
    /// file to the backup path failed
    SaveBackup,

    /// Returned by [`InPlaceFile::discard()`] if attempting to delete the
    /// temporary file failed
    Rmtemp,
}

impl InPlaceErrorKind {
    fn message(&self) -> &'static str {
        use InPlaceErrorKind::*;
        match self {
            Canonicalize => "failed to canonicalize path",
            CurrentDir => "failed to fetch current directory",
            EmptyBackup => "backup path is empty",
            GetMetadata => "failed to get metadata for path",
            Mktemp => "failed to create temporary file",
            NoFilename => "path does not have a filename",
            NoParent => "path does not have a parent directory",
            Open => "failed to open file for reading",
            SetMetadata => "failed to set metadata on temporary file",
            PersistTemp => "failed to save temporary file at path",
            SaveBackup => "failed to move file to backup path",
            Rmtemp => "failed to delete temporary file",
        }
    }
}

fn absolutize(filepath: &Path) -> Result<PathBuf, InPlaceError> {
    if filepath.is_absolute() {
        Ok(filepath.into())
    } else {
        let cwd = std::env::current_dir().map_err(InPlaceError::cwd)?;
        Ok(cwd.join(filepath))
    }
}

fn mktemp(filepath: &Path) -> Result<NamedTempFile, InPlaceError> {
    let dirpath = filepath.parent().ok_or_else(InPlaceError::no_parent)?;
    Builder::new()
        .prefix("._in_place-")
        .tempfile_in(dirpath)
        .map_err(InPlaceError::mktemp)
}

fn copystats(src: &Path, dest: &File, follow_symlinks: bool) -> Result<(), InPlaceError> {
    let md = if follow_symlinks {
        metadata(src)
    } else {
        symlink_metadata(src)
    }
    .map_err(InPlaceError::get_metadata)?;
    if !md.is_symlink() {
        dest.set_permissions(md.permissions())
            .map_err(InPlaceError::set_metadata)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
