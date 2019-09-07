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

pub mod simple;


use std::str::FromStr;
use crate::errors::{XcpError};

#[derive(Debug, Clone)]
pub enum Drivers {
    Simple
}

impl FromStr for Drivers {
    type Err = XcpError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "simple" => Ok(Drivers::Simple),
            _ => Err(XcpError::UnknownDriver { driver: s.to_owned() }.into()),
        }
    }

}
