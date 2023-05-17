use same_file::is_same_file;
use std::error;
use std::ffi::OsString;
use std::fmt;
use std::fs::{metadata, rename, File};
use std::io;
use std::path::{Path, PathBuf};
use tempfile::{Builder, NamedTempFile, PersistError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InPlace {
    path: PathBuf,
    backup: Option<Backup>,
    follow_symlinks: bool,
}

impl InPlace {
    pub fn new<P: AsRef<Path>>(path: P) -> InPlace {
        InPlace {
            path: path.as_ref().into(),
            backup: None,
            follow_symlinks: true,
        }
    }

    pub fn backup(&mut self, backup: Backup) -> &mut Self {
        self.backup = Some(backup);
        self
    }

    pub fn no_backup(&mut self) -> &mut Self {
        self.backup = None;
        self
    }

    pub fn follow_symlinks(&mut self, flag: bool) -> &mut Self {
        self.follow_symlinks = flag;
        self
    }

    pub fn open(&mut self) -> Result<InPlaceFile, OpenError> {
        let path = if self.follow_symlinks {
            self.path.canonicalize().map_err(OpenError::canonicalize)?
        } else {
            absolutize(&self.path)?
        };
        // Don't try to canonicalize backup_path, as it likely won't exist,
        // which would lead to an error
        let backup_path = match self.backup.as_ref() {
            Some(bkp) => Some(absolutize(&bkp.apply(&path)?)?),
            None => None,
        };
        if let Some(bkp) = backup_path.as_ref() {
            if is_same_file(&path, bkp).unwrap_or(false) {
                // If an error occurs, it's because either the backup path
                // doesn't exist (and thus can't equal `path`) or there was an
                // error opening `path` (and thus we can wait for opening
                // `reader` lower down to fail).
                return Err(OpenError::same_file());
            }
        }
        let writer = mktemp(&path)?;
        copystats(&path, writer.as_file())?;
        let reader = File::open(&path).map_err(OpenError::open)?;
        Ok(InPlaceFile {
            reader,
            writer,
            path,
            backup_path,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Backup {
    Path(PathBuf),
    FileName(OsString),
    AppendExtension(OsString),
    SetExtension(OsString),
}

impl Backup {
    fn apply(&self, path: &Path) -> Result<PathBuf, OpenError> {
        match self {
            Backup::Path(p) => {
                if p == Path::new("") {
                    Err(OpenError::empty_backup())
                } else {
                    Ok(p.clone())
                }
            }
            Backup::FileName(fname) => {
                if fname.is_empty() {
                    Err(OpenError::empty_backup())
                } else {
                    Ok(path.with_file_name(fname))
                }
            }
            Backup::AppendExtension(ext) => match path.file_name() {
                Some(fname) => {
                    let mut fname = fname.to_os_string();
                    fname.push(ext);
                    Ok(path.with_file_name(&fname))
                }
                None => Err(OpenError::no_filename()),
            },
            Backup::SetExtension(ext) => Ok(path.with_extension(ext)),
        }
    }
}

#[derive(Debug)]
pub struct InPlaceFile {
    reader: File,
    writer: NamedTempFile,
    path: PathBuf,
    backup_path: Option<PathBuf>,
}

impl InPlaceFile {
    pub fn reader(&self) -> &File {
        &self.reader
    }

    pub fn writer(&self) -> &File {
        self.writer.as_file()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn backup_path(&self) -> Option<&Path> {
        self.backup_path.as_deref()
    }

    pub fn save(self) -> Result<(), SaveError> {
        if let Some(bp) = self.backup_path.as_ref() {
            rename(&self.path, bp).map_err(SaveError::save_backup)?;
        }
        #[cfg(windows)]
        if self.backup_path.is_none() {
            std::fs::remove_file(&self.path).map_err(SaveError::pre_persist)?;
        }
        let r = self.writer.persist(&self.path).map_err(SaveError::persist);
        if r.is_err() {
            if let Some(bp) = self.backup_path.as_ref() {
                let _ = rename(bp, &self.path);
            }
        }
        r.map(|_| ())
    }

    pub fn discard(self) -> Result<(), DiscardError> {
        self.writer.close().map_err(DiscardError::rmtemp)
    }
}

#[derive(Debug)]
pub struct OpenError {
    kind: OpenErrorKind,
    source: Option<io::Error>,
}

impl OpenError {
    pub fn kind(&self) -> OpenErrorKind {
        self.kind
    }

    pub fn as_io_error(&self) -> Option<&io::Error> {
        self.source.as_ref()
    }

    pub fn into_io_error(self) -> Option<io::Error> {
        self.source
    }

    fn get_metadata(source: io::Error) -> OpenError {
        OpenError {
            kind: OpenErrorKind::GetMetadata,
            source: Some(source),
        }
    }

    fn set_metadata(source: io::Error) -> OpenError {
        OpenError {
            kind: OpenErrorKind::SetMetadata,
            source: Some(source),
        }
    }

    fn no_parent() -> OpenError {
        OpenError {
            kind: OpenErrorKind::NoParent,
            source: None,
        }
    }

    fn mktemp(source: io::Error) -> OpenError {
        OpenError {
            kind: OpenErrorKind::Mktemp,
            source: Some(source),
        }
    }

    fn canonicalize(source: io::Error) -> OpenError {
        OpenError {
            kind: OpenErrorKind::Canonicalize,
            source: Some(source),
        }
    }

    fn cwd(source: io::Error) -> OpenError {
        OpenError {
            kind: OpenErrorKind::CurrentDir,
            source: Some(source),
        }
    }

    fn open(source: io::Error) -> OpenError {
        OpenError {
            kind: OpenErrorKind::Open,
            source: Some(source),
        }
    }

    fn empty_backup() -> OpenError {
        OpenError {
            kind: OpenErrorKind::EmptyBackup,
            source: None,
        }
    }

    fn no_filename() -> OpenError {
        OpenError {
            kind: OpenErrorKind::NoFilename,
            source: None,
        }
    }

    fn same_file() -> OpenError {
        OpenError {
            kind: OpenErrorKind::SameFile,
            source: None,
        }
    }
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind.message())
    }
}

impl error::Error for OpenError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.source.as_ref().map(|e| e as &dyn error::Error)
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum OpenErrorKind {
    // Note that `Canonicalize` is the error kind encountered when the input
    // path doesn't exist.
    Canonicalize,
    CurrentDir,
    EmptyBackup,
    GetMetadata,
    Mktemp,
    NoFilename,
    NoParent,
    Open,
    SameFile,
    SetMetadata,
}

impl OpenErrorKind {
    fn message(&self) -> &'static str {
        use OpenErrorKind::*;
        match self {
            Canonicalize => "failed to canonicalize path",
            CurrentDir => "failed to fetch current directory",
            EmptyBackup => "backup path is empty",
            GetMetadata => "failed to get metadata for file",
            Mktemp => "failed to create temporary file",
            NoFilename => "path does not have a filename",
            NoParent => "path does not have a parent directory",
            Open => "failed to open file for reading",
            SameFile => "path and backup path point to same file",
            SetMetadata => "failed to set metadata on temporary file",
        }
    }
}

#[derive(Debug)]
pub struct SaveError {
    kind: SaveErrorKind,
    source: io::Error,
}

impl SaveError {
    pub fn kind(&self) -> SaveErrorKind {
        self.kind
    }

    pub fn as_io_error(&self) -> &io::Error {
        &self.source
    }

    pub fn into_io_error(self) -> io::Error {
        self.source
    }

    fn save_backup(source: io::Error) -> SaveError {
        SaveError {
            kind: SaveErrorKind::SaveBackup,
            source,
        }
    }

    #[cfg(windows)]
    fn pre_persist(source: io::Error) -> SaveError {
        SaveError {
            kind: SaveErrorKind::Rmpath,
            source,
        }
    }

    fn persist(source: PersistError) -> SaveError {
        SaveError {
            kind: SaveErrorKind::PersistTemp,
            source: source.error,
        }
    }
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind.message())
    }
}

impl error::Error for SaveError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum SaveErrorKind {
    PersistTemp,
    // This only occurs on Windows
    Rmpath,
    SaveBackup,
}

impl SaveErrorKind {
    fn message(&self) -> &'static str {
        use SaveErrorKind::*;
        match self {
            PersistTemp => "failed to save temporary file at path",
            Rmpath => "failed to remove file",
            SaveBackup => "failed to move file to backup path",
        }
    }
}

#[derive(Debug)]
pub struct DiscardError {
    kind: DiscardErrorKind,
    source: io::Error,
}

impl DiscardError {
    pub fn kind(&self) -> DiscardErrorKind {
        self.kind
    }

    pub fn as_io_error(&self) -> &io::Error {
        &self.source
    }

    pub fn into_io_error(self) -> io::Error {
        self.source
    }

    fn rmtemp(source: io::Error) -> DiscardError {
        DiscardError {
            kind: DiscardErrorKind::Rmtemp,
            source,
        }
    }
}

impl fmt::Display for DiscardError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind.message())
    }
}

impl error::Error for DiscardError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum DiscardErrorKind {
    Rmtemp,
}

impl DiscardErrorKind {
    fn message(&self) -> &'static str {
        match self {
            DiscardErrorKind::Rmtemp => "failed to delete temporary file",
        }
    }
}

fn absolutize(filepath: &Path) -> Result<PathBuf, OpenError> {
    if filepath.is_absolute() {
        Ok(filepath.into())
    } else {
        let cwd = std::env::current_dir().map_err(OpenError::cwd)?;
        Ok(cwd.join(filepath))
    }
}

fn mktemp(filepath: &Path) -> Result<NamedTempFile, OpenError> {
    let dirpath = filepath.parent().ok_or_else(OpenError::no_parent)?;
    Builder::new()
        .prefix("._in_place-")
        .tempfile_in(dirpath)
        .map_err(OpenError::mktemp)
}

fn copystats(src: &Path, dest: &File) -> Result<(), OpenError> {
    // Don't bother with switching to symlink_metadata() when follow_symlinks
    // is false, as it seems (based on Python's shutil.copystats()) that
    // permissions can only be copied from a symlink if they're being copied to
    // another symlink, which our temp files are not.
    let perms = metadata(src)
        .map_err(OpenError::get_metadata)?
        .permissions();
    dest.set_permissions(perms).map_err(OpenError::set_metadata)
}

#[cfg(test)]
mod tests;
