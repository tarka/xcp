# xcp: An extended cp

`xcp` is a (partial) clone of the Unix `cp` command. It is not intended as a
full replacement, but as a companion utility with some more user-friendly
feedback and some optimisations that make sense under certain tasks (see
below).

[![Crates.io](https://img.shields.io/crates/v/xcp.svg?colorA=777777)](https://crates.io/crates/xcp) 
![Github Actions](https://github.com/tarka/xcp/actions/workflows/tests.yml/badge.svg)
[![Packaging status](https://repology.org/badge/tiny-repos/xcp.svg)](https://repology.org/project/xcp/versions)

*Warning*: `xcp` is currently beta-level software and almost certainly contains
bugs and unexpected or inconsistent behaviour. It probably shouldn't be used for
anything critical yet.

Please note that there are some known issues with copying files from virtual
filesystems (e.g. `/proc`, `/sys`). See [this LWN
article](https://lwn.net/Articles/846403/) for an overview of some of the
complexities of dealing with kernel-generated files.  This is a common problem
with file utilities which rely on random access; for example `rsync` has the
same issue.

## Installation

### Cargo

`xcp` can be installed directly from `crates.io` with:
```
cargo install xcp
```

### Arch Linux

[`xcp`](https://aur.archlinux.org/packages/xcp/) is available on the Arch Linux User Repository. If you use an AUR helper, you can execute a command such as this:
```
yay -S xcp
```

### NetBSD
[`xcp`](https://pkgsrc.se/sysutils/xcp) is available on NetBSD from the official repositories. To install it, simply run:
```
pkgin install xcp
```

## Features and Anti-Features

### Features

* Displays a progress-bar, both for directory and single file copies. This can
  be disabled with `--no-progress`.
* On Linux it uses `copy_file_range` call to copy files. This is the most
  efficient method of file-copying under Linux; in particular it is
  filesystem-aware, and can massively speed-up copies on network mounts by
  performing the copy operations server-side. However, unlike `copy_file_range`
  sparse files are detected and handled appropriately.
* Support for modern filesystem features such as [reflinks](https://btrfs.readthedocs.io/en/latest/Reflink.html).
* Optimised for 'modern' systems (i.e. multiple cores, copious RAM, and
  solid-state disks, especially ones connected into the main system bus,
  e.g. NVMe).
* Optional aggressive parallelism for systems with parallel IO. Quick
  experiments on a modern laptop suggest there may be benefits to parallel
  copies on NVMe disks. This is obviously highly system-dependent.
* Switchable 'drivers' to facilitate experimenting with alternative strategies
  for copy optimisation. Currently 2 drivers are available:
  * 'parfile': the previous hard-coded xcp copy method, which parallelises
    tree-walking and per-file copying. This is the default.
  * 'parblock': An experimental driver that parallelises copying at the block
    level. This has the potential for performance improvements in some
    architectures, but increases complexity. Testing is welcome.
* Non-Linux Unix-like OSs (OS X, *BSD) are supported via fall-back operation
  (although sparse-files are not yet supported in this case).
* Optionally understands `.gitignore` files to limit the copied directories.
* Optional native file-globbing.
* Optional checksum verification to detect copy errors caused by storage or
  memory issues. The checksum is calculated during the copy and verified by
  re-reading only the destination file. Uses xxHash for minimal performance
  impact.

### (Possible) future features

* Conversion of files to sparse where appropriate, as with `cp`'s
  `--sparse=always` flag.
* Aggressive sparseness detection with `lseek`.
* On non-Linux OSs sparse-files are not currenty supported but could be added if
  supported by the OS.

### Differences with `cp`

* Permissions, xattrs and ACLs are copied by default; this can be disabled with
  `--no-perms`.
* Virtual file copies are not supported; for example `/proc` and `/sys` files.
* Character files such as [sockets](https://man7.org/linux/man-pages/man7/unix.7.html) and
  [pipes](https://man7.org/linux/man-pages/man3/mkfifo.3.html) are copied as
  devices (i.e. via [mknod](https://man7.org/linux/man-pages/man2/mknod.2.html))
  rather than copying their contents as a stream.
* The `--reflink=never` option may silently perform a reflink operation
  regardless. This is due to the use of
  [copy_file_range](https://man7.org/linux/man-pages/man2/copy_file_range.2.html)
  which has no such override and may perform its own optimisations.
* `cp` 'simple' backups are not supported, only numbered.
* Some `cp` options are not available but may be added in the future.

## Performance

Benchmarks are mostly meaningless, but the following are results from a laptop
with an NVMe disk and in single-user mode. The target copy directory is a git
checkout of the Firefox codebase, having been recently gc'd (i.e. a single 4.1GB
pack file). `fstrim -va` and `echo 3 | sudo tee /proc/sys/vm/drop_caches` are
run before each test run to minimise SSD allocation performance interference.

Note: `xcp` is optimised for 'modern' systems with lots of RAM and solid-state
disks. In particular it is likely to perform worse on spinning disks unless they
are in highly parallel arrays.

### Local copy

* Single 4.1GB file copy, with the kernel cache dropped each run:
    * `cp`: ~6.2s
    * `xcp`: ~4.2s
* Single 4.1GB file copy, warmed cache (3 runs each):
    * `cp`: ~1.85s
    * `xcp`: ~1.7s
* Directory copy, kernel cache dropped each run:
    * `cp`: ~48s
    * `xcp`: ~56s
* Directory copy, warmed cache (3 runs each):
    * `cp`: ~6.9s
    * `xcp`: ~7.4s

### NFS copy

`xcp` uses `copy_file_range`, which is filesystem aware. On NFSv4 this will result
in the copy occurring server-side rather than transferring across the network. For
large files this can be a significant win:

* Single 4.1GB file on NFSv4 mount
    * `cp`: 6m18s
    * `xcp`: 0m37s

## Usage Examples

### Basic Copy

```bash
# Simple file copy
xcp source.txt dest.txt

# Recursive directory copy
xcp -r source_dir/ dest_dir/

# Copy with progress bar disabled
xcp --no-progress large_file.bin /mnt/backup/
```

### Checksum Verification

Use `--verify-checksum` to detect copy errors caused by hardware issues:

```bash
# Copy with checksum verification
xcp --verify-checksum important_file.bin backup.bin

# Recursive copy with verification
xcp -r --verify-checksum project/ /backup/project/

# Works with both drivers
xcp --driver=parblock --verify-checksum large_file.bin dest.bin
```

**How it works:**
- Checksum calculated during copy (using xxHash3 for speed)
- Destination file re-read to verify integrity
- Error returned immediately on mismatch (no retry)
- Works with both `parfile` and `parblock` drivers
- Sparse file optimization is disabled when checksum verification is enabled to
  ensure consistent hashing

**Performance:** ~2x overhead due to destination re-read (e.g., 34ms → 70ms for
50MB). Worthwhile for critical data where integrity matters.

**For mechanical hard drives (HDD):** Checksum verification may cause performance
issues due to the read-after-write pattern. If you experience hangs or slow
performance on HDDs, the verification should still work correctly but may be slower.
For maximum data integrity assurance, add `--fsync` to force data to disk before
verification (slower but guarantees correct checksums even in rare cache coherency
scenarios):

```bash
# Maximum integrity for critical data (slower on HDD)
xcp --verify-checksum --fsync critical_data.db backup.db
```

### Other Options

```bash
# Copy with specific number of workers
xcp --workers 8 -r large_dir/ backup/

# Use block-level parallelism
xcp --driver=parblock --block-size=4MB huge_file.bin dest.bin

# Respect .gitignore files
xcp -r --gitignore project/ backup/

# Sync to disk after each file
xcp --fsync critical_file.db backup.db
```
