
use std::{fs::{create_dir_all, File}, path::{PathBuf, Path}};

use anyhow::Result;
use clap::{self, CommandFactory, ValueEnum};
use clap_complete::{self as comp, Shell};
use clap_mangen::Man;
use libxcp::options;

fn write_manpage(dir: &Path) -> Result<()> {
    let opts = options::Opts::command();
    let man = Man::new(opts);
    let manfile = dir.join("xcp.1");
    let mut out = File::create(manfile)?;
    man.render(&mut out)?;
    Ok(())
}

fn write_completions(dir: &Path) -> Result<()> {
    let mut opts = options::Opts::command();
    for shell in Shell::value_variants() {
        comp::generate_to(
            *shell,
            &mut opts,
            "xcp",
            &dir,
        )?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let asset_dir  = PathBuf::from("../target/assets");
    create_dir_all(&asset_dir)?;

    write_manpage(&asset_dir)?;

    write_completions(&asset_dir)?;

    Ok(())
}
