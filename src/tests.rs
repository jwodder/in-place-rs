use super::*;
use assert_fs::fixture::TempDir;
use assert_fs::prelude::*;
use cfg_if::cfg_if;
use std::fs::{read_dir, read_link, remove_file};
use std::io;
use tmp_env::set_current_dir;

cfg_if! {
    if #[cfg(unix)] {
        use std::os::unix::fs::symlink;
    } else if #[cfg(windows)] {
        use std::os::windows::fs::symlink_file;
    }
}

static TEXT: &str = concat!(
    "'Twas brillig, and the slithy toves\n",
    "\tDid gyre and gimble in the wabe;\n",
    "All mimsy were the borogoves,\n",
    "\tAnd the mome raths outgrabe.\n",
);

fn listdir(dirpath: &Path) -> io::Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in read_dir(dirpath)? {
        files.push(entry?.file_name().to_string_lossy().into_owned());
    }
    files.sort();
    Ok(files)
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
        let mut inp = InPlace::new(&p).open().unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(swapcase(TEXT));
}

#[test]
fn novars() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let mut inp = InPlace::new(&p).open().unwrap();
        for line in (&mut inp.reader).lines() {
            writeln!(inp.writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(swapcase(TEXT));
}

#[test]
fn backup_ext() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let mut inp = InPlace::new(&p)
            .backup(Backup::AppendExtension("~".into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt", "file.txt~"]);
    p.assert(swapcase(TEXT));
    tmpdir.child("file.txt~").assert(TEXT);
}

#[test]
fn backup_filename() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let mut inp = InPlace::new(&p)
            .backup(Backup::FileName("backup.txt".into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(swapcase(TEXT));
    tmpdir.child("backup.txt").assert(TEXT);
}

#[test]
fn backup_path() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let mut inp = InPlace::new(&p)
            .backup(Backup::Path(tmpdir.child("backup.txt").path().into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(swapcase(TEXT));
    tmpdir.child("backup.txt").assert(TEXT);
}

/* TODO:
#[test]
fn test_empty_backup_ext() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let r = InPlace::new(&p).backup(Backup::AppendExtension("".into())).open();
        assert!(r.is_err());
        // Make more assertions about `r`
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(TEXT);
}
*/

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
        let mut inp = InPlace::new(&p).open().unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for (i, line) in reader.lines().enumerate() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
            if i == 2 {
                remove_file(&p).unwrap();
            }
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["file.txt"]);
    p.assert(swapcase(TEXT));
}

// Cannot delete open files on Windows
#[cfg(not(windows))]
#[test]
fn delete_backup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let mut inp = InPlace::new(&p)
            .backup(Backup::FileName("backup.txt".into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for (i, line) in reader.lines().enumerate() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
            if i == 2 {
                remove_file(&p).unwrap();
            }
        }
        let r = inp.save();
        assert!(r.is_err());
        // TODO: Assert more about r
    }
    assert!(listdir(&tmpdir).unwrap().is_empty());
}

#[test]
fn discard_nobackup() {
    let tmpdir = TempDir::new().unwrap();
    let p = tmpdir.child("file.txt");
    p.write_str(TEXT).unwrap();
    {
        let mut inp = InPlace::new(&p).open().unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
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
        let mut inp = InPlace::new(&p)
            .backup(Backup::FileName("backup.txt".into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.discard().unwrap();
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
        let mut inp = InPlace::new(&p)
            .backup(Backup::Path(bkp.path().into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(swapcase(TEXT));
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
        let mut inp = InPlace::new(&p)
            .backup(Backup::Path(bkp.path().into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
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
        let mut inp = InPlace::new(&p)
            .backup(Backup::Path("backup.txt".into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(swapcase(TEXT));
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
        let mut inp = InPlace::new("file.txt")
            .backup(Backup::Path("backup.txt".into()))
            .open()
            .unwrap();
        let _chdir = set_current_dir(&wrongdir);
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert!(listdir(&wrongdir).unwrap().is_empty());
    assert_eq!(listdir(&filedir).unwrap(), ["backup.txt", "file.txt"]);
    p.assert(swapcase(TEXT));
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
        let mut inp = InPlace::new("filedir/file.txt")
            .backup(Backup::Path("bkpdir/backup.txt".into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&filedir).unwrap(), ["file.txt"]);
    assert_eq!(listdir(&bkpdir).unwrap(), ["backup.txt"]);
    p.assert(swapcase(TEXT));
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
        let mut inp = InPlace::new("filedir/file.txt")
            .backup(Backup::Path("backup.txt".into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&filedir).unwrap(), ["file.txt"]);
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "filedir"]);
    p.assert(swapcase(TEXT));
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
        let mut inp = InPlace::new(&p)
            .backup(Backup::Path(not_a_file.path().into()))
            .open()
            .unwrap();
        writeln!(inp.writer, "This will be discarded.\n").unwrap();
        let r = inp.save();
        assert!(r.is_err());
        // TODO: Make more assertions about `r`
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
        let mut inp = InPlace::new(&p)
            .backup(Backup::Path(backup.path().into()))
            .open()
            .unwrap();
        writeln!(inp.writer, "This will be discarded.\n").unwrap();
        let r = inp.save();
        assert!(r.is_err());
        // TODO: Make more assertions about `r`
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
    // TODO: Assert more about `r`
    assert!(listdir(&tmpdir).unwrap().is_empty());
}

#[test]
fn symlink_nobackup() {
    let tmpdir = TempDir::new().unwrap();
    let realdir = tmpdir.child("real");
    realdir.create_dir_all().unwrap();
    let real = realdir.child("realfile.txt");
    real.write_str(TEXT).unwrap();
    let linkdir = tmpdir.child("link");
    linkdir.create_dir_all().unwrap();
    let link = linkdir.child("linkfile.txt");
    cfg_if! {
        if #[cfg(unix)] {
            symlink(Path::new("../real/realfile.txt"), &link).unwrap()
        } else if #[cfg(windows)] {
            if symlink_file(Path::new("..\\real\\realfile.txt"), &link).is_err() {
                // Assume symlinks aren't enabled for us and skip the test
                return;
            }
        } else {
            return;
        }
    }
    {
        let mut inp = InPlace::new(&link).open().unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert_eq!(listdir(&linkdir).unwrap(), ["linkfile.txt"]);
    assert!(link.is_symlink());
    cfg_if! {
        if #[cfg(unix)] {
            assert_eq!(read_link(&link).unwrap(), Path::new("../real/realfile.txt"));
        } else if #[cfg(windows)] {
            assert_eq!(read_link(&link).unwrap(), Path::new("..\\real\\realfile.txt"));
        }
    }
    real.assert(swapcase(TEXT));
    link.assert(swapcase(TEXT));
}

#[test]
fn symlink_backup_ext() {
    let tmpdir = TempDir::new().unwrap();
    let realdir = tmpdir.child("real");
    realdir.create_dir_all().unwrap();
    let real = realdir.child("realfile.txt");
    real.write_str(TEXT).unwrap();
    let linkdir = tmpdir.child("link");
    linkdir.create_dir_all().unwrap();
    let link = linkdir.child("linkfile.txt");
    cfg_if! {
        if #[cfg(unix)] {
            symlink(Path::new("../real/realfile.txt"), &link).unwrap()
        } else if #[cfg(windows)] {
            if symlink_file(Path::new("..\\real\\realfile.txt"), &link).is_err() {
                // Assume symlinks aren't enabled for us and skip the test
                return;
            }
        } else {
            return;
        }
    }
    {
        let mut inp = InPlace::new(&link)
            .backup(Backup::AppendExtension("~".into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert_eq!(
        listdir(&linkdir).unwrap(),
        ["linkfile.txt", "linkfile.txt~"]
    );
    assert!(link.is_symlink());
    cfg_if! {
        if #[cfg(unix)] {
            assert_eq!(read_link(&link).unwrap(), Path::new("../real/realfile.txt"));
        } else if #[cfg(windows)] {
            assert_eq!(read_link(&link).unwrap(), Path::new("..\\real\\realfile.txt"));
        }
    }
    real.assert(swapcase(TEXT));
    link.assert(swapcase(TEXT));
    linkdir.child("linkfile.txt~").assert(TEXT);
}

#[test]
fn symlink_backup() {
    let tmpdir = TempDir::new().unwrap();
    let realdir = tmpdir.child("real");
    realdir.create_dir_all().unwrap();
    let real = realdir.child("realfile.txt");
    real.write_str(TEXT).unwrap();
    let linkdir = tmpdir.child("link");
    linkdir.create_dir_all().unwrap();
    let link = linkdir.child("linkfile.txt");
    cfg_if! {
        if #[cfg(unix)] {
            symlink(Path::new("../real/realfile.txt"), &link).unwrap()
        } else if #[cfg(windows)] {
            if symlink_file(Path::new("..\\real\\realfile.txt"), &link).is_err() {
                // Assume symlinks aren't enabled for us and skip the test
                return;
            }
        } else {
            return;
        }
    }
    {
        let mut inp = InPlace::new(&link)
            .backup(Backup::Path(tmpdir.child("backup.txt").path().into()))
            .open()
            .unwrap();
        let reader = &mut inp.reader;
        let writer = &mut inp.writer;
        for line in reader.lines() {
            writeln!(writer, "{}", swapcase(&line.unwrap())).unwrap();
        }
        inp.save().unwrap();
    }
    assert_eq!(listdir(&tmpdir).unwrap(), ["backup.txt", "link", "real"]);
    assert_eq!(listdir(&realdir).unwrap(), ["realfile.txt"]);
    assert_eq!(listdir(&linkdir).unwrap(), ["linkfile.txt"]);
    assert!(link.is_symlink());
    cfg_if! {
        if #[cfg(unix)] {
            assert_eq!(read_link(&link).unwrap(), Path::new("../real/realfile.txt"));
        } else if #[cfg(windows)] {
            assert_eq!(read_link(&link).unwrap(), Path::new("..\\real\\realfile.txt"));
        }
    }
    real.assert(swapcase(TEXT));
    link.assert(swapcase(TEXT));
    tmpdir.child("backup.txt").assert(TEXT);
}
