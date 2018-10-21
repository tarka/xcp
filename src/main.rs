mod errors;
mod operations;
mod utils;

use simplelog::{Config, LevelFilter, TermLogger};
use std::io::ErrorKind as IOKind;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::errors::{io_err, Result, XcpError};
use crate::operations::{copy_single_file, copy_tree};

#[derive(Debug, StructOpt)]
#[structopt(
    name = "xcp",
    about = "Copy SOURCE to DEST, or multiple SOURCE(s) to DIRECTORY."
)]
pub struct Opts {
    /// Explain what is being done. Can be specified multiple times to
    /// increase logging.
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u64,

    /// Copy directories recursively
    #[structopt(short = "r", long = "recursive")]
    recursive: bool,

    /// Do not overwrite an exising file
    #[structopt(short = "n", long = "no-clobber")]
    noclobber: bool,

    #[structopt(parse(from_os_str))]
    source: PathBuf,

    #[structopt(parse(from_os_str))]
    dest: PathBuf,
}

fn check_and_copy_tree(opts: Opts) -> Result<()> {
    if opts.dest.exists() && !opts.dest.is_dir() {
        return Err(XcpError::InvalidDestination {
            msg: "Source is directory but target exists and is not a directory",
        }
        .into());
    }
    copy_tree(opts)
}

fn main() -> Result<()> {
    let opts = Opts::from_args();

    TermLogger::init(
        match opts.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        },
        Config::default(),
    )?;

    if !opts.source.exists() {
        return Err(io_err(IOKind::NotFound, "Source does not exist."));
    }

    if opts.source.is_file() {
        copy_single_file(opts)?;
    } else if opts.source.is_dir() {
        match opts.recursive {
            true => check_and_copy_tree(opts)?,
            false => {
                return Err(XcpError::InvalidSource {
                    msg: "Source is directory and --recursive not specified.",
                }
                .into())
            }
        }
    }

    Ok(())
}
