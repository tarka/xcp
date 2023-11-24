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

pub use anyhow::{Error, Result};

#[derive(Debug, thiserror::Error)]
pub enum XcpError {
    #[error("Invalid source: {0}")]
    InvalidSource(&'static str),

    #[error("Unsupported operation; this function should never be called on this OS.")]
    UnsupportedOperation,
}
