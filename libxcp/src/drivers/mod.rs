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

use crate::errors::{Result, XcpError};
use crate::options::Opts;

pub trait CopyDriver {
    fn copy_all(&self, sources: Vec<PathBuf>, dest: &Path, opts: Arc<Opts>) -> Result<()>;
    fn copy_single(&self, source: &Path, dest: &Path, opts: Arc<Opts>) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub enum Drivers {
    ParFile,
    #[cfg(feature = "parblock")]
    ParBlock,
}

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

pub fn pick_driver(opts: &Opts) -> Result<Box<dyn CopyDriver>> {
    let dopt = opts.driver.unwrap_or(Drivers::ParFile);

    let driver: Box<dyn CopyDriver> = match dopt {
        Drivers::ParFile => Box::new(parfile::Driver::new(opts)?),
        #[cfg(feature = "parblock")]
        Drivers::ParBlock => Box::new(parblock::Driver::new(opts)?),
    };

    Ok(driver)
}
