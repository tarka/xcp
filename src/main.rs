
mod errors;

use std::fs::{copy as fs_copy, File, Metadata};
use std::io::{Error as IOError, ErrorKind as IOKind};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

use crate::errors::{Result, Error, XcpError};


#[derive(Debug, StructOpt)]
#[structopt(name = "xcp",
            about = "Copy SOURCE to DEST, or multiple SOURCE(s) to DIRECTORY.")]
struct Opts {
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


fn to_err(kind: IOKind, desc: &str) -> Error {
    IOError::new(kind, desc).into()
}


fn copy_single_file(opts: &Opts) -> Result<()> {
    let dest = if opts.dest.is_dir() {
        let fname = opts.source.file_name().ok_or(XcpError::UnknownFilename)?;
        opts.dest.join(fname)
    } else {
        opts.dest.clone()
    };

    if dest.is_file() && opts.noclobber {
        return Err(to_err(IOKind::AlreadyExists, "Destination file exists and no-clobber is set."));
    }

    fs_copy(&opts.source, &dest)?;

    Ok(())
}


fn main() -> Result<()> {
    let opts = Opts::from_args();

    if ! opts.source.exists() {
        return Err(to_err(IOKind::NotFound, "Source does not exist."));
    }

    if opts.source.is_file() {
        copy_single_file(&opts)?;
    }

    Ok(())
}
