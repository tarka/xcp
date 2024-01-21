/*
 * Copyright © 2018-2019, Steve Smith <tarkasteve@gmail.com>
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

pub mod parfile;
#[cfg(feature = "parblock")]
pub mod parblock;

use std::path::{Path, PathBuf};
use std::result;
use std::str::FromStr;
use std::sync::Arc;

use crate::config::Config;
use crate::errors::{Result, XcpError};
use crate::operations::StatSender;

pub trait CopyDriver {
    fn new(config: Arc<Config>) -> Result<Self> where Self: Sized;
    fn copy_all(&self, sources: Vec<PathBuf>, dest: &Path, stats: Arc<dyn StatSender>) -> Result<()>;
    fn copy_single(&self, source: &Path, dest: &Path, stats: Arc<dyn StatSender>) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub enum Drivers {
    ParFile,
    #[cfg(feature = "parblock")]
    ParBlock,
}

// String conversion helper as a convenience for command-line parsing.
impl FromStr for Drivers {
    type Err = XcpError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "parfile" => Ok(Drivers::ParFile),
            #[cfg(feature = "parblock")]
            "parblock" => Ok(Drivers::ParBlock),
            _ => Err(XcpError::UnknownDriver(s.to_owned())),
        }
    }
}

pub fn load_driver(driver: &Drivers, config: &Arc<Config>) -> Result<Box<dyn CopyDriver>> {
    let driver_impl: Box<dyn CopyDriver> = match driver {
        Drivers::ParFile => Box::new(parfile::Driver::new(config.clone())?),
        #[cfg(feature = "parblock")]
        Drivers::ParBlock => Box::new(parblock::Driver::new(config.clone())?),
    };

    Ok(driver_impl)
}
