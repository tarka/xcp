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

mod options;
mod progress;

use std::path::PathBuf;
use std::{result, thread};
use std::sync::Arc;

use glob::{glob, Paths};
use libxcp::config::Config;
use libxcp::drivers::load_driver;
use libxcp::errors::{Result, XcpError};
use libxcp::feedback::{ChannelUpdater, StatusUpdate, StatusUpdater};
use log::{error, info};

use crate::options::Opts;

fn init_logging(opts: &Opts) -> Result<()> {
    use simplelog::{ColorChoice, Config, SimpleLogger, TermLogger, TerminalMode};

    TermLogger::init(
        opts.log_level(),
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    ).or_else(
        |_| SimpleLogger::init(opts.log_level(), Config::default())
    )?;

    Ok(())
}

// Expand a list of file-paths or glob-patterns into a list of concrete paths.
// FIXME: This currently eats non-existent files that are not
// globs. Should we convert empty glob results into errors?
fn expand_globs(patterns: &[String]) -> Result<Vec<PathBuf>> {
    let paths = patterns.iter()
        .map(|s| glob(s.as_str()))
        .collect::<result::Result<Vec<Paths>, _>>()?
        .iter_mut()
        // Force resolve each glob Paths iterator into a vector of the results...
        .map::<result::Result<Vec<PathBuf>, _>, _>(Iterator::collect)
        // And lift all the results up to the top.
        .collect::<result::Result<Vec<Vec<PathBuf>>, _>>()?
        .iter()
        .flat_map(ToOwned::to_owned)
        .collect::<Vec<PathBuf>>();

    Ok(paths)
}

fn expand_sources(source_list: &[String], opts: &Opts) -> Result<Vec<PathBuf>> {
    if opts.glob {
        expand_globs(source_list)
    } else {
        let pb = source_list.iter()
            .map(PathBuf::from)
            .collect::<Vec<PathBuf>>();
        Ok(pb)
    }
}

fn main() -> Result<()> {
    let opts = Opts::from_args()?;
    init_logging(&opts)?;

    let (dest, source_patterns) = opts
        .paths
        .split_last()
        .ok_or(XcpError::InvalidArguments("Insufficient arguments".to_string()))
        .map(|(d, s)| (PathBuf::from(d), s))?;


    let sources = expand_sources(source_patterns, &opts)?;
    if sources.is_empty() {
        return Err(XcpError::InvalidSource("No source files found.").into());
    } else if !dest.is_dir() {
        if sources.len() == 1 && sources[0].is_dir() && dest.exists() {
            return Err(XcpError::InvalidDestination("Cannot copy a directory to a file.").into());
        } else if sources.len() > 1 {
            return Err(XcpError::InvalidDestination("Multiple sources and destination is not a directory.").into());
        }
    }

    // Sanity-check all sources up-front
    for source in &sources {
        info!("Copying source {:?} to {:?}", source, dest);
        if !source.exists() {
            return Err(XcpError::InvalidSource("Source does not exist.").into());
        }

        if source.is_dir() && !opts.recursive {
            return Err(XcpError::InvalidSource("Source is directory and --recursive not specified.").into());
        }

        if source == &dest {
            return Err(XcpError::InvalidSource("Cannot copy a directory into itself").into());
        }
    }


    // ========== Start copy ============

    let config = Arc::new(Config::from(&opts));
    let driver = load_driver(opts.driver, &config)?;

    let updater = ChannelUpdater::new(&config);
    let stat_rx = updater.rx_channel();
    let stats: Arc<dyn StatusUpdater> = Arc::new(updater);

    let handle = thread::spawn(move || -> Result<()> {
        driver.copy(sources, &dest, stats)
    });


    // ========== Collect output and display ============

    let pb = progress::create_bar(&opts, 0)?;

    // Gather the results as we go; our end of the channel has been
    // moved to the driver call and will end when drained.
    for stat in stat_rx {
        match stat {
            StatusUpdate::Copied(v) => pb.inc(v),
            StatusUpdate::Size(v) => pb.inc_size(v),
            StatusUpdate::Error(e) => {
                // FIXME: Optional continue?
                error!("Received error: {}", e);
                return Err(e.into());
            }
        }
    }

    handle.join()
        .map_err(|_| XcpError::CopyError("Error during copy operation".to_string()))??;

    info!("Copy complete");
    pb.end();

    Ok(())
}
