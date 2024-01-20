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
//! [xcp]: https://crates.io/crates/xcp/

pub mod config;
pub mod drivers;
pub mod errors;
pub mod operations;
pub mod paths;
