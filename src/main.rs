
mod errors;

use std::fs::{File, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

use crate::errors::{Result, Error};


#[derive(Debug, StructOpt)]
#[structopt(name = "xcp",
            about = "Copy SOURCE to DEST, or multiple SOURCE(s) to DIRECTORY.")]
struct Opts {
    /// Explain what is being done
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,

    #[structopt(parse(from_os_str))]
    source: PathBuf,

    #[structopt(parse(from_os_str))]
    dest: PathBuf,
}


fn copy_file(opts: &Opts) -> Result<()> {

    Ok(())
}

fn main() -> Result<()> {
    let opts = Opts::from_args();

    if ! opts.source.exists() {
        let e = io::Error::new(io::ErrorKind::NotFound,
                               "Source file does not exist.");
        return Err(e.into());
    }

    if opts.source.is_file() {
        copy_file(&opts)?;
    }

    Ok(())
}
