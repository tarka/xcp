
use failure::{Error};

use std::process::Command;
use escargot::{CargoBuild, CargoError};

#[test]
fn basic() -> Result<(), Error>  {
    let mut bin = CargoBuild::new()
        .run()?
        .command();

    let out = bin
        .arg("--help")
        .output()?;

    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout)?;
    assert!(stdout.contains("Copy SOURCE to DEST"));

    Ok(())
}
