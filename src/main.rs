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
mod drivers;
mod options;
mod os;
mod progress;
mod utils;

use std::path::PathBuf;
use std::io::ErrorKind as IOKind;

use log::info;
use simplelog::{Config, LevelFilter, SimpleLogger, TermLogger, TerminalMode};
use structopt::StructOpt;

use crate::errors::{io_err, Result, XcpError};
use crate::drivers::{CopyDriver, Drivers};

fn main() -> Result<()> {
    let opts = options::Opts::from_args();

    let log_level = match opts.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    TermLogger::init(log_level, Config::default(), TerminalMode::Mixed)
        .or_else(|_| SimpleLogger::init(log_level, Config::default()))?;

    let dopt = opts.driver.unwrap_or(Drivers::Simple);
    let driver: &dyn CopyDriver = match dopt {
        Drivers::Simple => &drivers::simple::Driver{},
        Drivers::ParBlock => &drivers::parblock::Driver{},
    };

    let (dest, source_patterns) = opts.paths
        .split_last()
        .ok_or(XcpError::InvalidArguments{msg: "Insufficient arguments"})
        .map(|(d, s)| {
            (PathBuf::from(d), s)
        })?;

    // Do this check before expansion otherwise it could result in
    // unexpected behaviour when the a glob expands to a single file.
    if source_patterns.len() > 1 && !dest.is_dir() {
        return Err(XcpError::InvalidDestination {
            msg: "Multiple sources and destination is not a directory.",
        }
        .into());
    }

    let sources = options::expand_sources(source_patterns, &opts)?;
    if sources.is_empty() {
        return Err(io_err(IOKind::NotFound, "No source files found."));


    } else if sources.len() == 1 && dest.is_file() {
        // Special case; rename/overwrite existing file.
        if opts.noclobber {
            return Err(io_err(
                IOKind::AlreadyExists,
                "Destination file exists and --no-clobber is set.",
            ));
        }

        info!("Copying file {:?} to {:?}", sources[0], dest);
        driver.copy_single(&sources[0], dest, &opts)?;


    } else {

        // Sanity-check all sources up-front
        for source in &sources {
            info!("Copying source {:?} to {:?}", source, dest);
            if !source.exists() {
                return Err(io_err(IOKind::NotFound, "Source does not exist."));
            }

            if source.is_dir() && !opts.recursive {
                return Err(XcpError::InvalidSource {
                    msg: "Source is directory and --recursive not specified.",
                }.into())
            }

            if dest.exists() && !dest.is_dir() {
                return Err(XcpError::InvalidDestination {
                    msg: "Source is directory but target exists and is not a directory",
                }.into());
            }
        }

        driver.copy_all(sources, dest, &opts)?;
    }

    Ok(())
}
