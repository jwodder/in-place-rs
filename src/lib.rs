use std::error;
use std::ffi::{OsStr, OsString};
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
        let (path, backup_path) = if self.follow_symlinks {
            let backup_path = match self.backup.as_ref() {
                Some(bkp) => match bkp.apply(&self.path) {
                    Some(bp) => Some(absolutize(&bp)?),
                    None => return Err(OpenError::backup_path()),
                },
                None => None,
            };
            (
                self.path.canonicalize().map_err(OpenError::canonicalize)?,
                backup_path,
            )
        } else {
            todo!()
        };
        // TODO: Check that `path` and `backup_path` are not the same file
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
    fn apply(&self, path: &Path) -> Option<PathBuf> {
        match self {
            // TODO: Canonicalize this:
            Backup::Path(p) => Some(p.clone()),
            Backup::FileName(fname) => {
                (fname != OsStr::new("")).then(|| path.with_file_name(fname))
            }
            Backup::AppendExtension(ext) => {
                let mut fname = path.file_name()?.to_os_string();
                fname.push(ext);
                Some(path.with_file_name(&fname))
            }
            Backup::SetExtension(ext) => Some(path.with_extension(ext)),
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
            rename(&self.path, bp).map_err(SaveError::backup)?;
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

// TODO:
//impl Drop for InPlaceFile {
//    fn drop(&mut self) {
//       // discard() and ignore error
//    }
//}

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

    fn get_metadata(_e: io::Error) -> OpenError {
        todo!()
    }

    fn set_metadata(_e: io::Error) -> OpenError {
        todo!()
    }

    fn no_parent() -> OpenError {
        todo!()
    }

    fn mktemp(_e: io::Error) -> OpenError {
        todo!()
    }

    fn canonicalize(_e: io::Error) -> OpenError {
        todo!()
    }

    fn cwd(_e: io::Error) -> OpenError {
        todo!()
    }

    fn backup_path() -> OpenError {
        todo!()
    }

    fn open(_e: io::Error) -> OpenError {
        todo!()
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

    fn backup(_e: io::Error) -> SaveError {
        todo!()
    }

    fn persist(_e: PersistError) -> SaveError {
        todo!()
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

    fn rmtemp(_e: io::Error) -> DiscardError {
        todo!()
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OpenErrorKind;

impl OpenErrorKind {
    fn message(&self) -> &'static str {
        todo!()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SaveErrorKind;

impl SaveErrorKind {
    fn message(&self) -> &'static str {
        todo!()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct DiscardErrorKind;

impl DiscardErrorKind {
    fn message(&self) -> &'static str {
        todo!()
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
