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

use std::io::{Error as IOError};
use std::path::PathBuf;

use thiserror::Error;
pub use anyhow::{Result, Error};


#[derive(Debug, Error)]
pub enum XcpError {
    #[error("Unknown file-type: {0}")]
    UnknownFiletype(PathBuf),

    #[error("Unknown driver: {0}")]
    UnknownDriver(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(&'static str),

    #[error("Invalid source: {0}")]
    InvalidSource(&'static str),

    #[error("Invalid destination: {0}")]
    InvalidDestination(&'static str),

    #[error("Destination Exists: {0}, {1}")]
    DestinationExists(&'static str, PathBuf),

    #[error("IO Error: {0}")]
    IOError(IOError),

    #[error("Early shutdown: {0}")]
    EarlyShutdown(&'static str),

    #[error("Unsupported OS")]
    UnsupportedOS(&'static str),

    #[error("Unsupported operation; this function should never be called on this OS.")]
    UnsupportedOperation,
}
