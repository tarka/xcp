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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid source: {0}")]
    InvalidSource(&'static str),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    OSError(#[from] rustix::io::Errno),

    #[error("Unsupported operation; this function should never be called on this OS.")]
    UnsupportedOperation,

    #[error("Error processing callback: {0}")]
    CallbackError(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
