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

use crossbeam_channel as cbc;

use indicatif;

use crate::options::Opts;
use crate::errors::Result;

#[derive(Debug, Clone)]
pub enum StatusUpdate {
    Copied(u64),
    Size(u64),
}

impl StatusUpdate {
    fn set(&self, bytes: u64) -> StatusUpdate {
        match self {
            StatusUpdate::Copied(_) => StatusUpdate::Copied(bytes),
            StatusUpdate::Size(_) => StatusUpdate::Size(bytes),
        }
    }
    fn value(&self) -> u64 {
        match self {
            StatusUpdate::Copied(v) => *v,
            StatusUpdate::Size(v) => *v,
        }
    }
}

pub const BATCH_DEFAULT: u64 = 1024 * 1024 * 64;

pub trait Updater<T> {
    fn update(&mut self, update: T) -> Result<()>;
}

pub struct BatchUpdater {
    pub sender: Box<dyn Updater<Result<StatusUpdate>> + Send>,
    pub stat: StatusUpdate,
    pub batch_size: u64,
}


impl Updater<Result<u64>> for BatchUpdater {
    fn update(&mut self, status: Result<u64>) -> Result<()> {
        match status {
            Ok(bytes) => {
                let curr = self.stat.value() + bytes;
                self.stat = self.stat.set(curr);

                if curr >= self.batch_size {
                    self.sender.update(Ok(self.stat.clone()))?;
                    self.stat = self.stat.set(0);
                }
            }
            Err(e) => {
                self.sender.update(Err(e))?;
            }
        }
        Ok(())
    }
}


impl Updater<Result<StatusUpdate>> for cbc::Sender<Result<StatusUpdate>> {
    fn update(&mut self, update: Result<StatusUpdate>) -> Result<()> {
        Ok(self.send(update)?)
    }
}


pub struct NopUpdater {}

impl Updater<Result<StatusUpdate>> for NopUpdater {
    fn update(&mut self, _update: Result<StatusUpdate>) -> Result<()> {
        Ok(())
    }
}


pub struct ProgressUpdater {
    pub pb: ProgressBar,
    pub written: u64,
}

impl Updater<Result<StatusUpdate>> for ProgressUpdater {
    fn update(&mut self, update: Result<StatusUpdate>) -> Result<()> {
        if let Ok(StatusUpdate::Copied(bytes)) = update {
            self.written += bytes;
            self.pb.set_position(self.written);
        }
        Ok(())
    }
}


pub enum ProgressBar {
    Visual(indicatif::ProgressBar),
    Nop,
}

impl ProgressBar {
    pub fn new(opts: &Opts, size: u64) -> ProgressBar {
        match opts.noprogress {
            true => ProgressBar::Nop,
            false => iprogress_bar(size)
        }
    }

    pub fn set_size(&self, size: u64) {
        match self {
            ProgressBar::Visual(pb) => pb.set_length(size),
            ProgressBar::Nop => {}
        }
    }

    pub fn set_position(&self, size: u64) {
        match self {
            ProgressBar::Visual(pb) => pb.set_position(size),
            ProgressBar::Nop => {}
        }
    }

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


pub fn iprogress_bar(size: u64) -> ProgressBar {
    let ipb = indicatif::ProgressBar::new(size)
        .with_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:80.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .progress_chars("#>-"),
        );
    ProgressBar::Visual(ipb)
}
