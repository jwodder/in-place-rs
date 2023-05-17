use super::*;
use assert_fs::fixture::TempDir;
use assert_fs::prelude::*;
use std::fs::{read_dir, read_link, remove_file};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Component;
use tmp_env::set_current_dir;

static TEXT: &str = concat!(
    "'Twas brillig, and the slithy toves\n",
    "\tDid gyre and gimble in the wabe;\n",
    "All mimsy were the borogoves,\n",
    "\tAnd the mome raths outgrabe.\n",
);

static SWAPPED_TEXT: &str = concat!(
    "'tWAS BRILLIG, AND THE SLITHY TOVES\n",
    "\tdID GYRE AND GIMBLE IN THE WABE;\n",
    "aLL MIMSY WERE THE BOROGOVES,\n",
    "\taND THE MOME RATHS OUTGRABE.\n",
);

fn listdir(dirpath: &Path) -> io::Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in read_dir(dirpath)? {
        files.push(entry?.file_name().to_string_lossy().into_owned());
    }
    files.sort();
    Ok(files)
}

#[cfg(unix)]
fn mklink(target: &Path, link: &Path) -> io::Result<bool> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(true)
}

#[cfg(windows)]
fn mklink(target: &Path, link: &Path) -> io::Result<bool> {
    // If this errors, assume symlinks aren't enabled for us on this system and
    // skip the test
    Ok(std::os::windows::fs::symlink_file(target, link).is_ok())
}

#[cfg(all(not(unix), not(windows)))]
fn mklink(_: &Path, _: &Path) -> io::Result<bool> {
    // Whatever this is, assume it doesn't have symlinks
    Ok(false)
}

fn swapcase(s: &str) -> String {
    s.chars()
        .map(|ch| {
            if ch.is_ascii_lowercase() {
                ch.to_ascii_uppercase()
            } else {
                ch.to_ascii_lowercase()
            }
        })
        .collect()
}

#[test]
fn nobackup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p).open().unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(SWAPPED_TEXT);
}

#[test]
fn nobackup_bufwriter() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p).open().unwrap();
        let reader = BufReader::new(inp.reader());
        {
            let mut writer = BufWriter::new(inp.writer());
            for line in reader.lines() {
                writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
            }
            writer.flush().unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(SWAPPED_TEXT);
}

#[test]
fn backup_ext() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::AppendExtension("~".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt", "file.txt~"]);
    p.assert(SWAPPED_TEXT);
    tmpdir.child("file.txt~").assert(TEXT);
}

#[test]
fn backup_set_ext() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::SetExtension("bak".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.bak", "file.txt"]);
    p.assert(SWAPPED_TEXT);
    tmpdir.child("file.bak").assert(TEXT);
}

#[test]
fn backup_filename() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::FileName("backup.txt".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(SWAPPED_TEXT);
    tmpdir.child("backup.txt").assert(TEXT);
}

#[test]
fn backup_path() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::Path(tmpdir.child("backup.txt").path().into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(SWAPPED_TEXT);
    tmpdir.child("backup.txt").assert(TEXT);
}

#[test]
fn empty_backup_path() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let r = InPlace::new(&p).backup(Backup::Path("".into())).open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::EmptyBackup);
    assert_eq!(e.to_string(), "backup path is empty");
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn empty_backup_filename() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let r = InPlace::new(&p).backup(Backup::FileName("".into())).open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::EmptyBackup);
    assert_eq!(e.to_string(), "backup path is empty");
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn append_empty_backup_ext() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let r = InPlace::new(&p)
        .backup(Backup::AppendExtension("".into()))
        .open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::SameFile);
    assert_eq!(e.to_string(), "path and backup path point to same file");
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn set_same_backup_ext() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let r = InPlace::new(&p)
        .backup(Backup::SetExtension("txt".into()))
        .open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::SameFile);
    assert_eq!(e.to_string(), "path and backup path point to same file");
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn same_backup_filename() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let r = InPlace::new(&p)
        .backup(Backup::FileName("file.txt".into()))
        .open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::SameFile);
    assert_eq!(e.to_string(), "path and backup path point to same file");
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn same_backup_path() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let r = InPlace::new(&p)
        .backup(Backup::Path(p.path().into()))
        .open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::SameFile);
    assert_eq!(e.to_string(), "path and backup path point to same file");
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn nop_nobackup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p).open().unwrap();
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert("");
}

// Cannot delete open files on Windows
#[cfg(not(windows))]
#[test]
fn delete_nobackup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p).open().unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for (i, line) in reader.lines().enumerate() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
            if i == 2 {
                remove_file(&p).unwrap();
            }
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(SWAPPED_TEXT);
}

// Cannot delete open files on Windows
#[cfg(not(windows))]
#[test]
fn delete_backup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::FileName("backup.txt".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for (i, line) in reader.lines().enumerate() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
            if i == 2 {
                remove_file(&p).unwrap();
            }
        }
        let r = inp.save();
        assert!(r.is_err());
        let e = r.unwrap_err();
        assert_eq!(e.kind(), SaveErrorKind::SaveBackup);
        assert_eq!(e.to_string(), "failed to move file to backup path");
    }
    assert!(listdir(&tmpdir).unwrap().is_empty());
}

#[test]
fn discard_nobackup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p).open().unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.discard().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn discard_backup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::FileName("backup.txt".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.discard().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn drop_nobackup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p).open().unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn drop_backup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::FileName("backup.txt".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn overwrite_backup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let bkp = tmpdir.child("backup.txt");
    bkp.write_str("This is not the file you are looking for.\n")
        .unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::Path(bkp.path().into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(SWAPPED_TEXT);
    bkp.assert(TEXT);
}

#[test]
fn discard_overwrite_backup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let bkp = tmpdir.child("backup.txt");
    bkp.write_str("This is not the file you are looking for.\n")
        .unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::Path(bkp.path().into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.discard().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(TEXT);
    bkp.assert("This is not the file you are looking for.\n");
}

#[test]
fn prechdir_backup() {
    let tmpdir = TempDir::new().unwrap();
    let _chdir = set_current_dir(&tmpdir);
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new("file.txt")
            .backup(Backup::Path("backup.txt".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(SWAPPED_TEXT);
    tmpdir.child("backup.txt").assert(TEXT);
}

#[test]
fn postchdir_backup() {
    // Assert that changing directory after opening an InPlaceFile works
    let tmpdir = TempDir::new().unwrap();
    let filedir = tmpdir.child("filedir");
    filedir.create_dir_all().unwrap();
    let wrongdir = tmpdir.child("wrongdir");
    wrongdir.create_dir_all().unwrap();
    let p = filedir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let _chdir = set_current_dir(&filedir);
    {
        let inp = InPlace::new("file.txt")
            .backup(Backup::Path("backup.txt".into()))
            .open()
            .unwrap();
        let _chdir = set_current_dir(&wrongdir);
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert!(listdir(&wrongdir).unwrap().is_empty());
    assert_eq!(listdir(&filedir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(SWAPPED_TEXT);
    filedir.child("backup.txt").assert(TEXT);
}

#[test]
fn different_dir_backup() {
    let tmpdir = TempDir::new().unwrap();
    let _chdir = set_current_dir(&tmpdir);
    let filedir = tmpdir.child("filedir");
    filedir.create_dir_all().unwrap();
    let bkpdir = tmpdir.child("bkpdir");
    bkpdir.create_dir_all().unwrap();
    let p = filedir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new("filedir/file.txt")
            .backup(Backup::Path("bkpdir/backup.txt".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&filedir).unwrap(), ["file.txt"]);
    assert_eq!(listdir(&bkpdir).unwrap(), ["backup.txt"]);
    p.assert(SWAPPED_TEXT);
    bkpdir.child("backup.txt").assert(TEXT);
}

#[test]
fn different_dir_file_backup() {
    // Assert that if the input filepath contains a directory component and the
    // backup path does not, the backup file will be created in the current
    // directory
    let tmpdir = TempDir::new().unwrap();
    let _chdir = set_current_dir(&tmpdir);
    let filedir = tmpdir.child("filedir");
    filedir.create_dir_all().unwrap();
    let p = filedir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let inp = InPlace::new("filedir/file.txt")
            .backup(Backup::Path("backup.txt".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&filedir).unwrap(), ["file.txt"]);
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "filedir"]);
    p.assert(SWAPPED_TEXT);
    tmpdir.child("backup.txt").assert(TEXT);
}

#[test]
fn backup_dirpath() {
    // Assert that using a path to a directory as the backup path raises an
    // error when closing
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let not_a_file = tmpdir.child("not-a-file");
    not_a_file.create_dir_all().unwrap();
    {
        let inp = InPlace::new(&p)
            .backup(Backup::Path(not_a_file.path().into()))
            .open()
            .unwrap();
        writeln!(inp.writer(), "This will be discarded.\n").unwrap();
        let r = inp.save();
        assert!(r.is_err());
        let e = r.unwrap_err();
        assert_eq!(e.kind(), SaveErrorKind::SaveBackup);
        assert_eq!(e.to_string(), "failed to move file to backup path");
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt", "not-a-file"]);
    assert!(listdir(&not_a_file).unwrap().is_empty());
    p.assert(TEXT);
}

#[test]
fn backup_nosuchdir() {
    // Assert that using a path to a file in a nonexistent directory as the
    // backup path raises an error when closing
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let backup = tmpdir.child("nonexistent").child("backup.txt");
    {
        let inp = InPlace::new(&p)
            .backup(Backup::Path(backup.path().into()))
            .open()
            .unwrap();
        writeln!(inp.writer(), "This will be discarded.\n").unwrap();
        let r = inp.save();
        assert!(r.is_err());
        let e = r.unwrap_err();
        assert_eq!(e.kind(), SaveErrorKind::SaveBackup);
        assert_eq!(e.to_string(), "failed to move file to backup path");
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[test]
fn edit_nonexistent() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    let r = InPlace::new(p).open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::Canonicalize);
    assert_eq!(e.to_string(), "failed to canonicalize path");
    assert!(listdir(&tmpdir).unwrap().is_empty());
}

#[test]
fn symlink_nobackup() {
    let tmpdir = TempDir::new().unwrap();
    let realdir = tmpdir.child("realdir");
    realdir.create_dir_all().unwrap();
    let realfile = realdir.child("realfile.txt");
    realfile.write_str(TEXT).unwrap();
    let linkdir = tmpdir.child("linkdir");
    linkdir.create_dir_all().unwrap();
    let linkfile = linkdir.child("linkfile.txt");
    let target = PathBuf::from_iter(["..", "realdir", "realfile.txt"]);
    if !mklink(&target, &linkfile).unwrap() {
        // No symlinks; skip test
        return;
    }
    {
        let inp = InPlace::new(&linkfile).open().unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert_eq!(listdir(&linkdir).unwrap(), ["linkfile.txt"]);
    assert!(!realfile.is_symlink());
    realfile.assert(SWAPPED_TEXT);
    assert!(linkfile.is_symlink());
    assert_eq!(read_link(&linkfile).unwrap(), target);
    linkfile.assert(SWAPPED_TEXT);
}

#[test]
fn symlink_backup_ext() {
    let tmpdir = TempDir::new().unwrap();
    let realdir = tmpdir.child("realdir");
    realdir.create_dir_all().unwrap();
    let realfile = realdir.child("realfile.txt");
    realfile.write_str(TEXT).unwrap();
    let linkdir = tmpdir.child("linkdir");
    linkdir.create_dir_all().unwrap();
    let linkfile = linkdir.child("linkfile.txt");
    let target = PathBuf::from_iter(["..", "realdir", "realfile.txt"]);
    if !mklink(&target, &linkfile).unwrap() {
        // No symlinks; skip test
        return;
    }
    {
        let inp = InPlace::new(&linkfile)
            .backup(Backup::AppendExtension("~".into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(
        listdir(&realdir).unwrap(),
        ["realfile.txt", "realfile.txt~"]
    );
    assert!(!realfile.is_symlink());
    realfile.assert(SWAPPED_TEXT);
    assert!(!realdir.child("realfile.txt~").is_symlink());
    realdir.child("realfile.txt~").assert(TEXT);
    assert_eq!(listdir(&linkdir).unwrap(), ["linkfile.txt"]);
    assert!(linkfile.is_symlink());
    assert_eq!(read_link(&linkfile).unwrap(), target);
    linkfile.assert(SWAPPED_TEXT);
}

#[test]
fn symlink_backup() {
    let tmpdir = TempDir::new().unwrap();
    let realdir = tmpdir.child("realdir");
    realdir.create_dir_all().unwrap();
    let realfile = realdir.child("realfile.txt");
    realfile.write_str(TEXT).unwrap();
    let linkdir = tmpdir.child("linkdir");
    linkdir.create_dir_all().unwrap();
    let linkfile = linkdir.child("linkfile.txt");
    let target = PathBuf::from_iter(["..", "realdir", "realfile.txt"]);
    if !mklink(&target, &linkfile).unwrap() {
        // No symlinks; skip test
        return;
    }
    {
        let inp = InPlace::new(&linkfile)
            .backup(Backup::Path(tmpdir.child("backup.txt").path().into()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(
        listdir(&tmpdir).unwrap(),
        ["backup.txt", "linkdir", "realdir"]
    );
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert!(!realfile.is_symlink());
    realfile.assert(SWAPPED_TEXT);
    assert_eq!(listdir(&linkdir).unwrap(), ["linkfile.txt"]);
    assert!(linkfile.is_symlink());
    assert_eq!(read_link(&linkfile).unwrap(), target);
    linkfile.assert(SWAPPED_TEXT);
    assert!(!tmpdir.child("backup.txt").is_symlink());
    tmpdir.child("backup.txt").assert(TEXT);
}

#[test]
fn no_follow_symlink_nobackup() {
    let tmpdir = TempDir::new().unwrap();
    let realdir = tmpdir.child("realdir");
    realdir.create_dir_all().unwrap();
    let realfile = realdir.child("realfile.txt");
    realfile.write_str(TEXT).unwrap();
    let linkdir = tmpdir.child("linkdir");
    linkdir.create_dir_all().unwrap();
    let linkfile = linkdir.child("linkfile.txt");
    let target = PathBuf::from_iter(["..", "realdir", "realfile.txt"]);
    if !mklink(&target, &linkfile).unwrap() {
        // No symlinks; skip test
        return;
    }
    {
        let inp = InPlace::new(&linkfile)
            .follow_symlinks(false)
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert_eq!(listdir(&linkdir).unwrap(), ["linkfile.txt"]);
    assert!(!realfile.is_symlink());
    realfile.assert(TEXT);
    assert!(!linkfile.is_symlink());
    linkfile.assert(SWAPPED_TEXT);
}

#[test]
fn no_follow_symlink_backup_ext() {
    let tmpdir = TempDir::new().unwrap();
    let realdir = tmpdir.child("realdir");
    realdir.create_dir_all().unwrap();
    let realfile = realdir.child("realfile.txt");
    realfile.write_str(TEXT).unwrap();
    let linkdir = tmpdir.child("linkdir");
    linkdir.create_dir_all().unwrap();
    let linkfile = linkdir.child("linkfile.txt");
    let target = PathBuf::from_iter(["..", "realdir", "realfile.txt"]);
    if !mklink(&target, &linkfile).unwrap() {
        // No symlinks; skip test
        return;
    }
    {
        let inp = InPlace::new(&linkfile)
            .backup(Backup::AppendExtension("~".into()))
            .follow_symlinks(false)
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert!(!realfile.is_symlink());
    realfile.assert(TEXT);
    assert_eq!(
        listdir(&linkdir).unwrap(),
        ["linkfile.txt", "linkfile.txt~"]
    );
    assert!(!linkfile.is_symlink());
    linkfile.assert(SWAPPED_TEXT);
    assert!(linkdir.child("linkfile.txt~").is_symlink());
    assert_eq!(read_link(linkdir.child("linkfile.txt~")).unwrap(), target);
}

#[test]
fn no_follow_symlink_backup() {
    let tmpdir = TempDir::new().unwrap();
    let realdir = tmpdir.child("realdir");
    realdir.create_dir_all().unwrap();
    let realfile = realdir.child("realfile.txt");
    realfile.write_str(TEXT).unwrap();
    let linkdir = tmpdir.child("linkdir");
    linkdir.create_dir_all().unwrap();
    let linkfile = linkdir.child("linkfile.txt");
    let target = PathBuf::from_iter(["..", "realdir", "realfile.txt"]);
    if !mklink(&target, &linkfile).unwrap() {
        // No symlinks; skip test
        return;
    }
    {
        let inp = InPlace::new(&linkfile)
            .backup(Backup::Path(tmpdir.child("backup.txt").path().into()))
            .follow_symlinks(false)
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(
        listdir(&tmpdir).unwrap(),
        ["backup.txt", "linkdir", "realdir"]
    );
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert!(!realfile.is_symlink());
    realfile.assert(TEXT);
    assert_eq!(listdir(&linkdir).unwrap(), ["linkfile.txt"]);
    assert!(!linkfile.is_symlink());
    linkfile.assert(SWAPPED_TEXT);
    assert!(tmpdir.child("backup.txt").is_symlink());
    assert_eq!(read_link(tmpdir.child("backup.txt")).unwrap(), target);
}

#[test]
fn backup_is_symlink() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let realdir = tmpdir.child("realdir");
    realdir.create_dir_all().unwrap();
    let realfile = realdir.child("realfile.txt");
    realfile.write_str("This is a symlinked file.\n").unwrap();
    let linkdir = tmpdir.child("linkdir");
    linkdir.create_dir_all().unwrap();
    let linkfile = linkdir.child("linkfile.txt");
    let target = PathBuf::from_iter(["..", "realdir", "realfile.txt"]);
    if !mklink(&target, &linkfile).unwrap() {
        // No symlinks; skip test
        return;
    }
    {
        let inp = InPlace::new(&p)
            .backup(Backup::Path(linkfile.to_path_buf()))
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(
        listdir(&tmpdir).unwrap(),
        ["file.txt", "linkdir", "realdir"]
    );
    assert!(!p.is_symlink());
    p.assert(SWAPPED_TEXT);
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert!(!realfile.is_symlink());
    realfile.assert("This is a symlinked file.\n");
    assert_eq!(listdir(&linkdir).unwrap(), ["linkfile.txt"]);
    assert!(!linkfile.is_symlink());
    linkfile.assert(TEXT);
}

#[test]
fn backup_is_symlink_nofollow() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let realdir = tmpdir.child("realdir");
    realdir.create_dir_all().unwrap();
    let realfile = realdir.child("realfile.txt");
    realfile.write_str("This is a symlinked file.\n").unwrap();
    let linkdir = tmpdir.child("linkdir");
    linkdir.create_dir_all().unwrap();
    let linkfile = linkdir.child("linkfile.txt");
    let target = PathBuf::from_iter(["..", "realdir", "realfile.txt"]);
    if !mklink(&target, &linkfile).unwrap() {
        // No symlinks; skip test
        return;
    }
    {
        let inp = InPlace::new(&p)
            .backup(Backup::Path(linkfile.to_path_buf()))
            .follow_symlinks(false)
            .open()
            .unwrap();
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(
        listdir(&tmpdir).unwrap(),
        ["file.txt", "linkdir", "realdir"]
    );
    assert!(!p.is_symlink());
    p.assert(SWAPPED_TEXT);
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert!(!realfile.is_symlink());
    realfile.assert("This is a symlinked file.\n");
    assert_eq!(listdir(&linkdir).unwrap(), ["linkfile.txt"]);
    assert!(!linkfile.is_symlink());
    linkfile.assert(TEXT);
}

#[test]
fn no_parent() {
    let r = InPlace::new(PathBuf::from_iter([Component::RootDir])).open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::NoParent);
    assert_eq!(e.to_string(), "path does not have a parent directory");
}

#[cfg(unix)]
#[test]
fn unwritable_dir() {
    use std::fs::{set_permissions, Permissions};
    use std::os::unix::fs::PermissionsExt;
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    set_permissions(tmpdir.path(), Permissions::from_mode(0o555)).unwrap();
    let r = InPlace::new(&p).open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::Mktemp);
    assert_eq!(e.to_string(), "failed to create temporary file");
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}

#[cfg(unix)]
#[test]
fn unreadable_file() {
    use std::fs::{set_permissions, Permissions};
    use std::os::unix::fs::PermissionsExt;
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    set_permissions(p.path(), Permissions::from_mode(0o000)).unwrap();
    let r = InPlace::new(&p).open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::Open);
    assert_eq!(e.to_string(), "failed to open file for reading");
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
}

#[test]
fn file_links_to_backup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    let backup = tmpdir.child("backup.txt");
    backup.write_str(TEXT).unwrap();
    let target = Path::new("backup.txt");
    if !mklink(target, &p).unwrap() {
        // No symlinks; skip test
        return;
    }
    let r = InPlace::new(&p)
        .backup(Backup::Path(backup.to_path_buf()))
        .open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::SameFile);
    assert_eq!(e.to_string(), "path and backup path point to same file");
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    assert!(p.is_symlink());
    assert_eq!(read_link(&p).unwrap(), target);
    assert!(!backup.is_symlink());
    backup.assert(TEXT);
}

#[test]
fn bug_file_links_to_backup_nofollow() {
    // Bug: If the input path is a symlink to the backup path and
    // `follow_links` is `false`, `same_file` will produce a SameFile error,
    // even though without the check editing would work smoothly.
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    let backup = tmpdir.child("backup.txt");
    backup.write_str(TEXT).unwrap();
    let target = Path::new("backup.txt");
    if !mklink(target, &p).unwrap() {
        // No symlinks; skip test
        return;
    }
    let r = InPlace::new(&p)
        .backup(Backup::Path(backup.to_path_buf()))
        .follow_symlinks(false)
        .open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::SameFile);
    assert_eq!(e.to_string(), "path and backup path point to same file");
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    assert!(p.is_symlink());
    assert_eq!(read_link(&p).unwrap(), target);
    assert!(!backup.is_symlink());
    backup.assert(TEXT);
}

#[test]
fn bug_backup_links_to_file() {
    // Bug: If the backup path is a symlink to the input path, `same_file` will
    // produce a SameFile error, even though without the check editing would
    // work smoothly.
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    let backup = tmpdir.child("backup.txt");
    let target = Path::new("file.txt");
    if !mklink(target, &backup).unwrap() {
        // No symlinks; skip test
        return;
    }
    let r = InPlace::new(&p)
        .backup(Backup::Path(backup.to_path_buf()))
        .follow_symlinks(false)
        .open();
    assert!(r.is_err());
    let e = r.unwrap_err();
    assert_eq!(e.kind(), OpenErrorKind::SameFile);
    assert_eq!(e.to_string(), "path and backup path point to same file");
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    assert!(!p.is_symlink());
    p.assert(TEXT);
    assert!(backup.is_symlink());
    assert_eq!(read_link(&backup).unwrap(), target);
}
