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

//! Support for runtime feedback of copy progress.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use crossbeam_channel as cbc;

use crate::config::Config;
use crate::errors::{Result, XcpError};

#[derive(Debug)]
pub enum StatusUpdate {
    Copied(u64),
    Size(u64),
    Error(XcpError)
}

pub trait StatusUpdater: Sync + Send {
    fn send(&self, update: StatusUpdate) -> Result<()>;
}


pub struct ChannelUpdater {
    chan_tx: cbc::Sender<StatusUpdate>,
    chan_rx: cbc::Receiver<StatusUpdate>,
    config: Arc<Config>,
    sent: AtomicU64,
}

impl ChannelUpdater {
    /// Create a new ChannelUpdater, inclluding the channels.
    pub fn new(config: &Arc<Config>) -> ChannelUpdater {
        let (chan_tx, chan_rx) = cbc::unbounded();
        ChannelUpdater {
            chan_tx,
            chan_rx,
            config: config.clone(),
            sent: AtomicU64::new(0),
        }
    }

    /// Retrieve a clone of the receive end of the update
    /// channel. Note: As ChannelUpdater is consumed by the driver
    /// call you should call this before then; e.g:
    ///
    /// ```
    /// # use std::sync::Arc;
    /// use libxcp::config::Config;
    /// use libxcp::feedback::{ChannelUpdater, StatusUpdater};
    ///
    /// let config = Arc::new(Config::default());
    /// let updater = ChannelUpdater::new(&config);
    /// let stat_rx = updater.rx_channel();
    /// let stats: Arc<dyn StatusUpdater> = Arc::new(updater);
    /// ```
    pub fn rx_channel(&self) -> cbc::Receiver<StatusUpdate> {
        self.chan_rx.clone()
    }
}

impl StatusUpdater for ChannelUpdater {
    // Wrapper around channel-send that groups updates together
    fn send(&self, update: StatusUpdate) -> Result<()> {
        if let StatusUpdate::Copied(bytes) = update {
            // Avoid saturating the queue with small writes
            let bsize = self.config.block_size;
            let prev_written = self.sent.fetch_add(bytes, Ordering::Relaxed);
            if ((prev_written + bytes) / bsize) > (prev_written / bsize) {
                self.chan_tx.send(update)?;
            }
        } else {
            self.chan_tx.send(update)?;
        }
        Ok(())
    }
}


pub struct NoopUpdater;

impl StatusUpdater for NoopUpdater {
    fn send(&self, _update: StatusUpdate) -> Result<()> {
        Ok(())
    }
}
