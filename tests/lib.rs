use failure::Error;

use escargot::CargoBuild;
use std::fs::{create_dir_all, write, File};
use std::io::{Read, Write};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::tempdir;
use uuid::Uuid;

fn get_command() -> Result<Command, Error> {
    let cmd = CargoBuild::new().run()?.command();
    Ok(cmd)
}

fn run(args: &[&str]) -> Result<Output, Error> {
    let out = get_command()?.args(args).output()?;
    Ok(out)
}

fn tempdir_rel() -> Result<PathBuf, Error> {
    let uuid = Uuid::new_v4();
    let dir = PathBuf::from("target/").join(uuid.to_string());
    create_dir_all(&dir)?;
    Ok(dir)
}

fn create_file(path: &Path, text: &str) -> Result<(), Error> {
    let file = File::create(&path)?;
    write!(&file, "{}", text);
    Ok(())
}

fn file_contains(path: &Path, text: &str) -> Result<bool, Error> {
    let mut dest = File::open(path)?;
    let mut buf = String::new();
    dest.read_to_string(&mut buf)?;

    Ok(buf == text)
}

#[test]
fn basic_help() -> Result<(), Error> {
    let out = run(&["--help"])?;

    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout)?;
    assert!(stdout.contains("Copy SOURCE to DEST"));

    Ok(())
}

#[test]
fn no_args() -> Result<(), Error> {
    let out = run(&[])?;

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("The following required arguments were not provided"));

    Ok(())
}

#[test]
fn source_missing() -> Result<(), Error> {
    let out = run(&["/this/should/not/exist", "/dev/null"])?;

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("No source files found"));

    Ok(())
}

#[test]
fn dest_file_exists() -> Result<(), Error> {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        File::create(&source_path)?;
        File::create(&dest_path)?;
    }
    let out = run(&[
        "--no-clobber",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])?;

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Destination file exists"));

    Ok(())
}

#[test]
fn dest_file_in_dir_exists() -> Result<(), Error> {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");

    {
        File::create(&source_path)?;
        File::create(&dir.path().join("dest.txt"))?;
    }

    let out = run(&[
        "--no-clobber",
        source_path.to_str().unwrap(),
        dir.path().to_str().unwrap(),
    ])?;

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Destination file exists"));

    Ok(())
}

#[test]
fn file_copy() -> Result<(), Error> {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text)?;

    let out = run(&[source_path.to_str().unwrap(), dest_path.to_str().unwrap()])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text)?);

    Ok(())
}

#[test]
fn file_copy_rel() -> Result<(), Error> {
    let dir = tempdir_rel()?;
    let source_path = dir.join("source.txt");
    let dest_path = dir.join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text)?;

    let out = run(&[source_path.to_str().unwrap(), dest_path.to_str().unwrap()])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text)?);

    Ok(())
}


#[test]
fn file_copy_multiple() -> Result<(), Error> {
    let dir = tempdir_rel()?;
    let dest = dir.join("dest");
    create_dir_all(&dest)?;

    let (f1, f2) = (dir.join("file1.txt"),
                    dir.join("file2.txt"));
    create_file(&f1, "test")?;
    create_file(&f2, "test")?;

    let out = run(&[
        "-vv",
        f1.to_str().unwrap(),
        f2.to_str().unwrap(),
        dest.to_str().unwrap()])?;

    assert!(out.status.success());
    assert!(dest.join("file1.txt").exists());
    assert!(dest.join("file2.txt").exists());

    Ok(())
}


#[test]
fn copy_empty_dir() -> Result<(), Error> {
    let dir = tempdir()?;

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path)?;

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base)?;

    let out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());

    assert!(dest_base.join("mydir").exists());
    assert!(dest_base.join("mydir").is_dir());

    Ok(())
}

#[test]
fn copy_all_dirs() -> Result<(), Error> {
    let dir = tempdir()?;

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path)?;
    create_dir_all(source_path.join("one/two/three/"))?;

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base)?;

    let out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/two/three/").exists());
    assert!(dest_base.join("mydir/one/two/three/").is_dir());

    Ok(())
}

#[test]
fn copy_all_dirs_rel() -> Result<(), Error> {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    create_dir_all(&source_path)?;
    create_dir_all(source_path.join("one/two/three/"))?;

    let dest_base = dir.join("dest");
    create_dir_all(&dest_base)?;

    let out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/two/three/").exists());
    assert!(dest_base.join("mydir/one/two/three/").is_dir());

    Ok(())
}

#[test]
fn copy_dirs_files() -> Result<(), Error> {
    let dir = tempdir()?;

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path)?;

    let mut p = source_path.clone();
    for d in ["one", "two", "three"].iter() {
        p.push(d);
        create_dir_all(&p)?;
        create_file(&p.join(format!("{}.txt", d)), d)?;
    }

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base)?;

    let out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/one.txt").is_file());
    assert!(dest_base.join("mydir/one/two/two.txt").is_file());
    assert!(dest_base.join("mydir/one/two/three/three.txt").is_file());

    Ok(())
}

#[test]
fn copy_dirs_overwrites() -> Result<(), Error> {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest");
    create_dir_all(&dest_base)?;
    let dest_file = dest_base.join("mydir/file.txt");

    let mut out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "orig")?);

    write(&source_file, "new content")?;
    assert!(file_contains(&source_file, "new content")?);

    out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "new content")?);

    Ok(())
}

#[test]
fn dir_copy_to_nonexistent_is_rename() -> Result<(), Error> {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest");
    let dest_file = dest_base.join("file.txt");

    let out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig")?);

    Ok(())
}

#[test]
fn dir_overwrite_with_noclobber() -> Result<(), Error> {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest");
    create_dir_all(&dest_base)?;
    let dest_file = dest_base.join("mydir/file.txt");

    let mut out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "orig")?);

    write(&source_file, "new content")?;
    assert!(file_contains(&source_file, "new content")?);

    out = run(&[
        "-r",
        "--no-clobber",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(!out.status.success());

    Ok(())
}


#[test]
fn dir_copy_containing_symlinks() -> Result<(), Error> {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    let source_rlink = source_path.join("link.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;
    symlink("file.txt", source_rlink)?;
    symlink("/etc/hosts", source_path.join("hosts"))?;

    let dest_base = dir.join("dest");
    let dest_file = dest_base.join("file.txt");
    let dest_rlink = source_path.join("link.txt");

    let out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(dest_rlink.symlink_metadata()?.file_type().is_symlink());
    assert!(dest_base.join("hosts").symlink_metadata()?.file_type().is_symlink());

    Ok(())
}


#[test]
fn dir_copy_with_hidden_file() -> Result<(), Error> {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join(".file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest");
    let dest_file = dest_base.join(".file.txt");

    let out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig")?);

    Ok(())
}

#[test]
fn dir_copy_with_hidden_dir() -> Result<(), Error> {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir/.hidden");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest/.hidden");
    let dest_file = dest_base.join("file.txt");

    let out = run(&[
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig")?);

    Ok(())
}


#[test]
fn dir_with_gitignore() -> Result<(), Error> {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    let ignore_file = source_path.join(".gitignore");
    let hidden_path = dir.join("mydir/.hidden");
    let hidden_file = hidden_path.join("file.txt");
    create_dir_all(&hidden_path)?;
    create_file(&source_file, "orig")?;
    create_file(&hidden_file, "orig")?;
    create_file(&ignore_file, "/.hidden\n")?;

    let dest_base = dir.join("dest");

    let out = run(&[
        "-r",
        "--gitignore",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_base.join("file.txt").exists());
    assert!(dest_base.join(".gitignore").exists());
    assert!(!dest_base.join(".hidden").exists());

    Ok(())
}


#[test]
fn copy_with_glob() -> Result<(), Error> {
    let dir = tempdir_rel()?;
    let dest = dir.join("dest");
    create_dir_all(&dest)?;

    let (f1, f2) = (dir.join("file1.txt"),
                    dir.join("file2.txt"));
    create_file(&f1, "test")?;
    create_file(&f2, "test")?;

    let out = run(&[
        dir.join("file*.txt").to_str().unwrap(),
        dest.to_str().unwrap()])?;

    assert!(out.status.success());
    assert!(dest.join("file1.txt").exists());
    assert!(dest.join("file2.txt").exists());

    Ok(())
}

