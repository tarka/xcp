mod errors;
mod operations;
mod utils;

use std::io::{ErrorKind as IOKind};
use std::path::PathBuf;
use structopt::StructOpt;

use crate::errors::{Result};
use crate::operations::copy_single_file;
use crate::utils::to_err;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "xcp",
    about = "Copy SOURCE to DEST, or multiple SOURCE(s) to DIRECTORY."
)]
pub struct Opts {
    /// Explain what is being done
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,

    /// Do not overwrite an exising file
    #[structopt(short = "n", long = "no-clobber")]
    noclobber: bool,

    #[structopt(parse(from_os_str))]
    source: PathBuf,

    #[structopt(parse(from_os_str))]
    dest: PathBuf,
}


fn main() -> Result<()> {
    let opts = Opts::from_args();

    if !opts.source.exists() {
        return Err(to_err(IOKind::NotFound, "Source does not exist."));
    }

    if opts.source.is_file() {
        copy_single_file(&opts)?;
    }

    Ok(())
}
