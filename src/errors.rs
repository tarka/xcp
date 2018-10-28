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

use core::result;
use failure::Fail;
use std::io::{Error as IOError, ErrorKind as IOKind};
use std::path::PathBuf;

#[derive(Debug, Fail)]
pub enum XcpError {
    #[fail(display = "Failed to find filename.")]
    UnknownFilename,

    #[fail(display = "Unknown file-type: {:?}", path)]
    UnknownFiletype { path: PathBuf },

    #[fail(display = "Invalid source: {}", msg)]
    InvalidSource { msg: &'static str },

    #[fail(display = "Invalid destination: {}", msg)]
    InvalidDestination { msg: &'static str },

    #[fail(display = "Destination Exists: {:?}", path)]
    DestinationExists { msg: &'static str, path: PathBuf },

    #[fail(display = "Early shutdown: {:?}", msg)]
    EarlyShutdown { msg: &'static str },
}

pub fn io_err(kind: IOKind, desc: &str) -> Error {
    IOError::new(kind, desc).into()
}

pub use failure::Error;
pub type Result<T> = result::Result<T, Error>;
