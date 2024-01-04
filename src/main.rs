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

mod drivers;
mod errors;
mod operations;
mod options;
mod progress;
mod utils;

use std::path::PathBuf;
use std::sync::Arc;
use libfs::is_same_file;
use log::info;
use simplelog::{ColorChoice, Config, LevelFilter, SimpleLogger, TermLogger, TerminalMode};

use crate::drivers::load_driver;
use crate::errors::{Result, XcpError};

fn main() -> Result<()> {
    let opts = Arc::new(options::parse_args()?);

    let log_level = match opts.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    TermLogger::init(
        log_level,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .or_else(|_| SimpleLogger::init(log_level, Config::default()))?;

    let driver = load_driver(&opts)?;

    let (dest, source_patterns) = opts
        .paths
        .split_last()
        .ok_or(XcpError::InvalidArguments("Insufficient arguments".to_string()))
        .map(|(d, s)| (PathBuf::from(d), s))?;

    // Do this check before expansion otherwise it could result in
    // unexpected behaviour when the a glob expands to a single file.
    if source_patterns.len() > 1 && !dest.is_dir() {
        return Err(XcpError::InvalidDestination(
            "Multiple sources and destination is not a directory.",
        )
        .into());
    }

    let sources = options::expand_sources(source_patterns, &opts)?;
    if sources.is_empty() {
        return Err(XcpError::InvalidSource("No source files found.").into());

    } else if sources.len() == 1 && dest.is_file() {
        // Special case; attemping to rename/overwrite existing file.
        if opts.no_clobber {
            return Err(XcpError::DestinationExists(
                "Destination file exists and --no-clobber is set.",
                dest,
            )
            .into());
        }

        // Special case: Attempt to overwrite a file with
        // itself. Always disallow for now.
        if is_same_file(&sources[0], &dest)? {
            return Err(XcpError::DestinationExists(
                "Source and destination is the same file.",
                dest,
            )
            .into());
        }

        info!("Copying file {:?} to {:?}", sources[0], dest);
        driver.copy_single(&sources[0], &dest)?;

    } else {
        // Sanity-check all sources up-front
        for source in &sources {
            info!("Copying source {:?} to {:?}", source, dest);
            if !source.exists() {
                return Err(XcpError::InvalidSource("Source does not exist.").into());
            }

            if source.is_dir() && !opts.recursive {
                return Err(XcpError::InvalidSource(
                    "Source is directory and --recursive not specified.",
                )
                .into());
            }

            if source == &dest {
                return Err(XcpError::InvalidSource("Cannot copy a directory into itself").into());
            }

            if dest.exists() && !dest.is_dir() {
                return Err(XcpError::InvalidDestination(
                    "Source is directory but target exists and is not a directory",
                )
                .into());
            }
        }

        driver.copy_all(sources, &dest)?;
    }

    Ok(())
}
