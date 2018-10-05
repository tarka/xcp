
use failure::{Error};

use std::fs::{File};
use std::io::{Read, Write};
use std::process::Command;
use tempfile::tempdir;
use escargot::{CargoBuild};

fn get_bin() -> Result<Command, Error>  {
    let bin = CargoBuild::new()
        .run()?
        .command();
    Ok(bin)
}

#[test]
fn basic_help() -> Result<(), Error>  {
    let out = get_bin()?
        .arg("--help")
        .output()?;

    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout)?;
    assert!(stdout.contains("Copy SOURCE to DEST"));

    Ok(())
}


#[test]
fn no_args() -> Result<(), Error>  {
    let out = get_bin()?
        .output()?;

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("The following required arguments were not provided"));

    Ok(())
}

#[test]
fn source_missing() -> Result<(), Error>  {
    let out = get_bin()?
        .arg("/this/should/not/exist")
        .arg("/dev/null")
        .output()?;

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Source does not exist."));

    Ok(())
}

#[test]
fn dest_file_exists() -> Result<(), Error>  {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        File::create(&source_path)?;
        File::create(&dest_path)?;
    }
    let out = get_bin()?
        .arg("--no-clobber")
        .arg(source_path.as_os_str())
        .arg(dest_path.as_os_str())
        .output()?;

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Destination file exists"));

    Ok(())
}

#[test]
fn dest_file_in_dir_exists() -> Result<(), Error>  {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");

    {
        File::create(&source_path)?;
        File::create(&dir.path().join("dest.txt"))?;
    }

    let out = get_bin()?
        .arg("--no-clobber")
        .arg(source_path.as_os_str())
        .arg(dir.path().as_os_str())
        .output()?;

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Destination file exists"));

    Ok(())
}

#[test]
fn file_copy() -> Result<(), Error>  {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    {
        let source = File::create(&source_path)?;
        write!(&source, "{}", text);
    }

    let out = get_bin()?
        .arg(source_path.as_os_str())
        .arg(dest_path.as_os_str())
        .output()?;

    assert!(out.status.success());

    let mut dest = File::open(dest_path)?;
    let mut buf = String::new();
    dest.read_to_string(&mut buf)?;

    assert!(buf == text);

    Ok(())
}

