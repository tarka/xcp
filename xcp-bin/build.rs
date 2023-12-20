
use std::{fs::{create_dir_all, File}, path::{PathBuf, Path}};

use anyhow::Result;
use clap::{self, CommandFactory};
use clap_mangen::Man;
use libxcp::options;

fn write_manpage(manfile: &Path) -> Result<()> {
    let opts = options::Opts::command();
    let man = Man::new(opts);
    let mut out = File::create(manfile)?;
    man.render(&mut out)?;
    Ok(())
}

fn main() -> Result<()> {
    let asset_dir  = PathBuf::from("../target/assets");
    create_dir_all(&asset_dir)?;

    let manfile = asset_dir.join("xcp.1");
    write_manpage(&manfile)?;

    Ok(())
}
