
use failure::{Error};

use std::process::Command;
use escargot::{CargoBuild, CargoError};

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
