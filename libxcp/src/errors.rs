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

//! Custom error types.

use std::path::PathBuf;

pub use anyhow::Result;

#[derive(Debug, thiserror::Error)]
pub enum XcpError {
    #[error("Error during copy: {0}")]
    CopyError(String),

    #[error("Destination Exists: {0}, {1}")]
    DestinationExists(&'static str, PathBuf),

    #[error("Early shutdown: {0}")]
    EarlyShutdown(&'static str),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Invalid destination: {0}")]
    InvalidDestination(&'static str),

    #[error("Invalid source: {0}")]
    InvalidSource(&'static str),

    #[error("Failed to reflink file and 'always' was specified: {0}")]
    ReflinkFailed(String),

    #[error("Unknown driver: {0}")]
    UnknownDriver(String),

    #[error("Unknown file-type: {0}")]
    UnknownFileType(PathBuf),

    #[error("Unsupported OS")]
    UnsupportedOS(&'static str),
}
