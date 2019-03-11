/*
 * Copyright © 2018, Steve Smith <tarkasteve@gmail.com>
 *
 * This program is free software: you can redistribute it and/or
 * modify it under the terms of the GNU General Public License version
 * 3 as published by the Free Software Foundation.
 *
 * This program is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

mod errors;
mod operations;
mod options;
mod os;
mod progress;
mod utils;

use log::info;
use simplelog::{Config, LevelFilter, SimpleLogger, TermLogger};
use std::io::ErrorKind as IOKind;
use structopt::StructOpt;


use crate::errors::{io_err, Result, XcpError};
use crate::options::Opts;
use crate::operations::{copy_single_file, copy_all};

fn main() -> Result<()> {
    let opts = Opts::from_args();

    let log_level = match opts.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    TermLogger::init(log_level, Config::default())
        .or_else(|_| SimpleLogger::init(log_level, Config::default()))?;

    // Do this check before expansion otherwise it could result in
    // unexpected behaviour when the a glob expands to a single file.
    if opts.source_list.len() > 1 && !opts.dest.is_dir() {
        return Err(XcpError::InvalidDestination {
            msg: "Multiple sources and destination is not a directory.",
        }
        .into());
    }

    let sources = utils::to_pathbufs(&opts)?;
    if sources.is_empty() {
        return Err(io_err(IOKind::NotFound, "No source files found."));

    } else if sources.len() == 1 && opts.dest.is_file() {
        // Special case; rename/overwrite.
        info!("Copying file {:?} to {:?}", sources[0], opts.dest);
        copy_single_file(&sources[0], &opts)?;

    } else {

        // Sanity-check all sources up-front
        for source in &sources {
            info!("Copying source {:?} to {:?}", source, opts.dest);
            if !source.exists() {
                return Err(io_err(IOKind::NotFound, "Source does not exist."));
            }

            if source.is_dir() && !opts.recursive {
                return Err(XcpError::InvalidSource {
                    msg: "Source is directory and --recursive not specified.",
                }.into())
            }

            if opts.dest.exists() && !opts.dest.is_dir() {
                return Err(XcpError::InvalidDestination {
                    msg: "Source is directory but target exists and is not a directory",
                }.into());
            }
        }

        copy_all(sources, &opts)?;
    }

    Ok(())
}
