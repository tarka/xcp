use std::fs::copy as fs_copy;
use std::io::{ErrorKind as IOKind};

use crate::{Opts};
use crate::errors::{Result, XcpError};
use crate::utils::{to_err};

pub fn copy_single_file(opts: &Opts) -> Result<()> {
    let dest = if opts.dest.is_dir() {
        let fname = opts.source.file_name().ok_or(XcpError::UnknownFilename)?;
        opts.dest.join(fname)
    } else {
        opts.dest.clone()
    };

    if dest.is_file() && opts.noclobber {
        return Err(to_err(
            IOKind::AlreadyExists,
            "Destination file exists and no-clobber is set.",
        ));
    }

    fs_copy(&opts.source, &dest)?;

    Ok(())
}
