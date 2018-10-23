mod errors;
mod operations;
mod progress;
mod utils;

use log::{info};
use simplelog::{Config, LevelFilter, TermLogger};
use std::io::ErrorKind as IOKind;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::errors::{io_err, Result, XcpError};
use crate::operations::{copy_single_file, copy_tree};

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    name = "xcp",
    about = "Copy SOURCE to DEST, or multiple SOURCE(s) to DIRECTORY.",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
pub struct Opts {
    /// Explain what is being done. Can be specified multiple times to
    /// increase logging.
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u64,

    /// Copy directories recursively
    #[structopt(short = "r", long = "recursive")]
    recursive: bool,

    /// Do not overwrite an existing file
    #[structopt(short = "n", long = "no-clobber")]
    noclobber: bool,

    /// Use .gitignore if present. NOTE: This is fairly basic at the
    /// moment, and only honours a .gitignore in the directory root
    /// for directory copies; global or sub-directory ignores are
    /// skipped.
    #[structopt(long = "gitignore")]
    gitignore: bool,

    /// Disable progress bar.
    #[structopt(long = "no-progress")]
    noprogress: bool,

    #[structopt(
        raw(required="true", min_values="1"),
        parse(from_os_str)
    )]
    source_list: Vec<PathBuf>,

    #[structopt(parse(from_os_str))]
    dest: PathBuf,
}

fn check_and_copy_tree(source: PathBuf, opts: &Opts) -> Result<()> {
    if opts.dest.exists() && !opts.dest.is_dir() {
        return Err(XcpError::InvalidDestination {
            msg: "Source is directory but target exists and is not a directory",
        }
        .into());
    }
    copy_tree(source, opts)
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

    if opts.source_list.len() > 1 && !opts.dest.is_dir() {
        return Err(XcpError::InvalidDestination {
            msg: "Multiple sources and destination is not a directory.",
        }.into())
    }

    for source in &opts.source_list {
        info!("Copying source {:?} to {:?}", source, opts.dest);
        if !source.exists() {
            return Err(io_err(IOKind::NotFound, "Source does not exist."));
        }

        if source.is_file() {
            copy_single_file(&source, &opts)?;

        } else if source.is_dir() {
            match opts.recursive {
                true => check_and_copy_tree(source.to_path_buf(), &opts)?,
                false => {
                    return Err(XcpError::InvalidSource {
                        msg: "Source is directory and --recursive not specified.",
                    }.into())
                }
            }
        }
    }

    Ok(())
}
