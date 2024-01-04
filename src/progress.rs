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

use crate::errors::Result;
use crate::options::Opts;

pub enum ProgressBar {
    Visual(indicatif::ProgressBar),
    Nop,
}

impl ProgressBar {
    pub fn new(opts: &Opts, size: u64) -> Result<ProgressBar> {
        match opts.no_progress {
            true => Ok(ProgressBar::Nop),
            false => iprogress_bar(size),
        }
    }

    #[allow(unused)]
    pub fn set_size(&self, size: u64) {
        match self {
            ProgressBar::Visual(pb) => pb.set_length(size),
            ProgressBar::Nop => {}
        }
    }

    pub fn inc_size(&self, size: u64) {
        match self {
            ProgressBar::Visual(pb) => pb.inc_length(size),
            ProgressBar::Nop => {}
        }
    }

    pub fn set_position(&self, size: u64) {
        match self {
            ProgressBar::Visual(pb) => pb.set_position(size),
            ProgressBar::Nop => {}
        }
    }

    #[allow(unused)]
    pub fn inc(&self, size: u64) {
        match self {
            ProgressBar::Visual(pb) => pb.inc(size),
            ProgressBar::Nop => {}
        }
    }

    pub fn end(&self) {
        match self {
            ProgressBar::Visual(pb) => pb.finish(),
            ProgressBar::Nop => {}
        }
    }
}

fn iprogress_bar(size: u64) -> Result<ProgressBar> {
    let ipb = indicatif::ProgressBar::new(size).with_style(
        indicatif::ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
            .progress_chars("#>-"),
    );
    Ok(ProgressBar::Visual(ipb))
}
