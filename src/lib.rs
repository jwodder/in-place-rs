#![allow(dead_code)]
use std::error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{metadata, rename, File};
use std::io::{
    self, BufRead, BufReader, BufWriter, IoSlice, IoSliceMut, Read, Seek, SeekFrom, Write,
};
use std::path::{Path, PathBuf};
use tempfile::{Builder, NamedTempFile, PersistError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InPlace {
    path: PathBuf,
    backup: Option<Backup>,
    move_first: bool,
    follow_symlinks: bool,
}

impl InPlace {
    pub fn new<P: AsRef<Path>>(path: P) -> InPlace {
        InPlace {
            path: path.as_ref().into(),
            backup: None,
            move_first: false,
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

    pub fn move_first(&mut self, flag: bool) -> &mut Self {
        self.move_first = flag;
        self
    }

    pub fn follow_symlinks(&mut self, flag: bool) -> &mut Self {
        self.follow_symlinks = flag;
        self
    }

    pub fn open(&mut self) -> Result<InPlaceFile, OpenError> {
        let (path, backup_path) = if !self.follow_symlinks {
            todo!()
        } else {
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
        };
        if self.move_first {
            todo!()
        } else {
            // TODO: Check that `path` and `backup_path` are not the same file
            let tmpfile = mktemp(&path)?;
            copystats(&path, tmpfile.as_file())?;
            let input = File::open(&path).map_err(OpenError::open)?;
            Ok(InPlaceFile {
                reader: InPlaceReader::new(input, path.clone()),
                writer: InPlaceWriter::new(tmpfile),
                path,
                backup_path,
            })
        }
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
    pub reader: InPlaceReader,
    pub writer: InPlaceWriter,
    path: PathBuf,
    backup_path: Option<PathBuf>,
}

impl InPlaceFile {
    /* // Consider removing these, as they can't be called while a mutable
     * borrow on reader of writer is extant.
    // Returns the path to the file that was opened for in-place editing
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn backup_path(&self) -> Option<&Path> {
        self.backup_path.as_deref()
    }
    */

    // TODO: Is this a good idea?
    //pub fn temp_path(&self) -> &Path {
    //    self.tmpfile.path()
    //}

    pub fn save(mut self) -> Result<(), SaveError> {
        let _ = self.writer.flush();
        dbg!(&self.path);
        dbg!(&self.backup_path);
        if let Some(bp) = self.backup_path.as_ref() {
            rename(&self.path, bp).map_err(SaveError::backup)?;
        }
        let r = self
            .writer
            .into_tempfile()
            .map_err(SaveError::into_tempfile)
            .and_then(|tmpfile| tmpfile.persist(&self.path).map_err(SaveError::persist));
        if r.is_err() {
            if let Some(bp) = self.backup_path.as_ref() {
                let _ = rename(bp, &self.path);
            }
        }
        r.map(|_| ())
    }

    pub fn discard(self) -> Result<(), DiscardError> {
        self.writer
            .into_tempfile()
            .map_err(DiscardError::into_tempfile)
            .and_then(|tmpfile| tmpfile.close().map_err(DiscardError::rmtemp))
    }
}

// TODO:
//impl Drop for InPlaceFile {
//    fn drop(&mut self) {
//       // discard() and ignore error
//    }
//}

#[derive(Debug)]
pub struct InPlaceReader {
    inner: BufReader<File>,
    path: PathBuf,
}

impl InPlaceReader {
    fn new(file: File, path: PathBuf) -> Self {
        Self {
            inner: BufReader::new(file),
            path,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn as_file(&self) -> &File {
        self.inner.get_ref()
    }

    pub fn as_mut_file(&mut self) -> &mut File {
        self.inner.get_mut()
    }
}

impl Read for InPlaceReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.inner.read_exact(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.inner.read_to_string(buf)
    }
}

impl BufRead for InPlaceReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt);
    }
}

impl Seek for InPlaceReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        self.inner.stream_position()
    }
}

#[derive(Debug)]
pub struct InPlaceWriter(BufWriter<NamedTempFile>);

impl InPlaceWriter {
    fn new(file: NamedTempFile) -> Self {
        InPlaceWriter(BufWriter::new(file))
    }

    fn into_tempfile(self) -> Result<NamedTempFile, io::IntoInnerError<BufWriter<NamedTempFile>>> {
        self.0.into_inner()
    }

    pub fn path(&self) -> &Path {
        self.0.get_ref().path()
    }

    pub fn as_file(&self) -> &File {
        self.0.get_ref().as_file()
    }

    pub fn as_mut_file(&mut self) -> &mut File {
        self.0.get_mut().as_file_mut()
    }
}

impl Write for InPlaceWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.write_all(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Seek for InPlaceWriter {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.0.seek(pos)
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

    fn into_tempfile(_e: io::IntoInnerError<BufWriter<NamedTempFile>>) -> SaveError {
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

    fn into_tempfile(_e: io::IntoInnerError<BufWriter<NamedTempFile>>) -> DiscardError {
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
