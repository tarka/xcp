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

use crate::options::Opts;

use libxcp::errors::Result;

struct NoopBar;

struct VisualBar {
    bar: indicatif::ProgressBar,
}

pub trait ProgressBar {
    #[allow(unused)]
    fn set_size(&self, size: u64);
    fn inc_size(&self, size: u64);
    fn inc(&self, size: u64);
    fn end(&self);
}


impl ProgressBar for NoopBar {
    fn set_size(&self, _size: u64) {
    }
    fn inc_size(&self, _size: u64) {
    }
    fn inc(&self, _size: u64) {
    }
    fn end(&self) {
    }
}

impl ProgressBar for VisualBar {
    fn set_size(&self, size: u64) {
        self.bar.set_length(size);
    }

    fn inc_size(&self, size: u64) {
        self.bar.inc_length(size);
    }

    fn inc(&self, size: u64) {
        self.bar.inc(size);
    }

    fn end(&self) {
        self.bar.finish();
    }
}

impl VisualBar {
    fn new(size: u64) -> Result<Self> {
        let bar = indicatif::ProgressBar::new(size).with_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                .progress_chars("#>-"),
        );
        Ok(Self { bar })
    }
}

pub fn create_bar(opts: &Opts, size: u64) -> Result<Box<dyn ProgressBar>> {
    if opts.no_progress {
        Ok(Box::new(NoopBar {}))
    } else {
        Ok(Box::new(VisualBar::new(size)?))
    }
}
