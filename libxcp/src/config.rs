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

use crate::operations::Reflink;

#[derive(Clone, Debug, Default)]
pub struct Config {
    /// Number of parallel workers. 0 means use the number of logical
    /// CPUs.
    pub workers: usize,

    /// Block size for operations.
    pub block_size: u64,

    /// Use .gitignore if present.
    ///
    /// NOTE: This is fairly basic at the moment, and only honours a
    /// .gitignore in the directory root for each source directory;
    /// global or sub-directory ignores are skipped.
    pub gitignore: bool,

    /// Do not overwrite existing files
    pub no_clobber: bool,

    /// Do not copy the file permissions.
    pub no_perms: bool,

    /// Target should not be a directory.
    ///
    /// Analogous to cp's no-target-directory. Expected behavior is that when
    /// copying a directory to another directory, instead of creating a sub-folder
    /// in target, overwrite target.
    pub no_target_directory: bool,

    /// Sync each file to disk after writing.
    pub fsync: bool,

    /// Reflink options.
    ///
    /// Whether and how to use reflinks. 'auto' (the default) will
    /// attempt to reflink and fallback to a copy if it is not
    /// possible, 'always' will return an error if it cannot reflink,
    /// and 'never' will always perform a full data copy.
    pub reflink: Reflink,
}

impl Config {
    pub(crate) fn num_workers(&self) -> usize {
        if self.workers == 0 {
            num_cpus::get()
        } else {
            self.workers
        }
    }
}
