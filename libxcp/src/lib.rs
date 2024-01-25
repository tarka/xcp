/*
 * Copyright © 2024, Steve Smith <tarkasteve@gmail.com>
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

//! `libxcp` is a high-level file-copy engine. It has a support for
//! multi-threading, fine-grained progress feedback, pluggable
//! drivers, and `.gitignore` filters. `libxcp` is the core
//! functionality of the [xcp] command-line utility.
//!
//! # Usage example
//!
//!     # use std::path::PathBuf;
//!     # use std::sync::Arc;
//!     # use std::thread;
//!     # use tempfile::TempDir;
//!     #
//!     use libxcp::config::Config;
//!     use libxcp::errors::XcpError;
//!     use libxcp::feedback::{ChannelUpdater, StatusUpdater, StatusUpdate};
//!     use libxcp::drivers::{Drivers, load_driver};
//!
//!     let sources = vec![PathBuf::from("src")];
//!     let dest = TempDir::new().unwrap();
//!
//!     let config = Arc::new(Config::default());
//!     let updater = ChannelUpdater::new(&config);
//!     // The ChannelUpdater is consumed by the driver (so it
//!     // is properly closed on completion). Retrieve our end
//!     // of the connection before then.
//!     let stat_rx = updater.rx_channel();
//!     let stats: Arc<dyn StatusUpdater> = Arc::new(updater);
//!
//!     let driver = load_driver(Drivers::ParFile, &config).unwrap();
//!
//!     // As we want realtime updates via the ChannelUpdater the
//!     // copy operation should run in the background.
//!     let handle = thread::spawn(move || {
//!         driver.copy_all(sources, dest.path(), stats)
//!     });
//!
//!     // Gather the results as we go; our end of the channel has been
//!     // moved to the driver call and will end when drained.
//!     for stat in stat_rx {
//!         match stat {
//!             StatusUpdate::Copied(v) => {
//!                 println!("Copied {} bytes", v);
//!             },
//!             StatusUpdate::Size(v) => {
//!                 println!("Size update: {}", v);
//!             },
//!             StatusUpdate::Error(e) => {
//!                 panic!("Error during copy: {}", e);
//!             }
//!         }
//!     }
//!
//!     handle.join()
//!         .map_err(|_| XcpError::CopyError("Error during copy operation".to_string()))
//!         .unwrap().unwrap();
//!
//!     println!("Copy complete");
//!
//! [xcp]: https://crates.io/crates/xcp/

pub mod config;
pub mod drivers;
pub mod errors;
pub mod feedback;

// Internal
mod operations;
mod paths;

#[cfg(test)]
#[allow(unused)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::thread;

    use tempfile::TempDir;

    use crate::errors::{Result, XcpError};
    use crate::config::Config;
    use crate::feedback::{ChannelUpdater, StatusUpdater, StatusUpdate};
    use crate::drivers::{Drivers, load_driver};

    #[test]
    fn simple_usage_test() -> Result<()> {
        let sources = vec![PathBuf::from("src")];
        let dest = TempDir::new()?;

        let config = Arc::new(Config::default());
        let updater = ChannelUpdater::new(&config);
        let stat_rx = updater.rx_channel();
        let stats: Arc<dyn StatusUpdater> = Arc::new(updater);

        let driver = load_driver(Drivers::ParFile, &config)?;

        let handle = thread::spawn(move || {
            driver.copy_all(sources, dest.path(), stats)
        });

        // Gather the results as we go; our end of the channel has been
        // moved to the driver call and will end when drained.
        for stat in stat_rx {
            match stat {
                StatusUpdate::Copied(v) => {
                    println!("Copied {} bytes", v);
                },
                StatusUpdate::Size(v) => {
                    println!("Size update: {}", v);
                },
                StatusUpdate::Error(e) => {
                    println!("Error during copy: {}", e);
                    return Err(e.into());
                }
            }
        }

        handle.join()
            .map_err(|_| XcpError::CopyError("Error during copy operation".to_string()))??;

        println!("Copy complete");

        Ok(())
    }
}
