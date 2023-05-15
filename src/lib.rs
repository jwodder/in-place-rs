#![allow(dead_code)]
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{
    self, BufRead, BufReader, BufWriter, IoSlice, IoSliceMut, Read, Seek, SeekFrom, Write,
};
use std::path::{Path, PathBuf};

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
        todo!()
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
pub struct InPlaceFile;

impl InPlaceFile {
    pub fn reader(&mut self) -> &mut InPlaceReader {
        todo!()
    }

    pub fn writer(&mut self) -> &mut InPlaceWriter {
        todo!()
    }

    pub fn save(self) -> Result<(), SaveError> {
        todo!()
    }

    pub fn discard(self) -> Result<(), DiscardError> {
        todo!()
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

// TODO: Add wrappers for nightly-only methods that BufReader defines as well
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
pub struct InPlaceWriter {
    inner: BufWriter<File>,
    path: PathBuf,
}

impl InPlaceWriter {
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

// TODO: Add wrappers for nightly-only methods that BufWriter defines as well
impl Write for InPlaceWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.inner.write_all(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Seek for InPlaceWriter {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscardError;
