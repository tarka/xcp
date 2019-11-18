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
pub mod parblock;

use std::path::{PathBuf};
use std::result;
use std::str::FromStr;

use crate::options::Opts;
use crate::errors::{Result, XcpError};


pub trait CopyDriver {
    fn copy_all(&self, sources: Vec<PathBuf>, dest: PathBuf, opts: &Opts) -> Result<()>;
    fn copy_single(&self, source: &PathBuf, dest: PathBuf, opts: &Opts) -> Result<()>;
}


#[derive(Debug, Clone, Copy)]
pub enum Drivers {
    ParFile,
    ParBlock,
}

impl FromStr for Drivers {
    type Err = XcpError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s {
            "parfile" => Ok(Drivers::ParFile),
            "parblock" => Ok(Drivers::ParBlock),
            _ => Err(XcpError::UnknownDriver(s.to_owned()).into()),
        }
    }

}
