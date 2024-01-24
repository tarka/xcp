# libxcp: High-level file-copy engine

`libxcp` is a high-level file-copy engine. It has a support for multi-threading,
fine-grained progress feedback, pluggable drivers, and `.gitignore` filters.
`libxcp` is the core functionality of the [xcp](https://crates.io/crates/xcp)
command-line utility.

[![Crates.io](https://img.shields.io/crates/v/xcp.svg?colorA=777777)](https://crates.io/crates/libxcp)
[![doc.rs](https://docs.rs/libxcp/badge.svg)](https://docs.rs/libxcp)
![Github Actions](https://github.com/tarka/xcp/actions/workflows/tests.yml/badge.svg)
[![CircleCI](https://circleci.com/gh/tarka/xcp.svg?style=shield)](https://circleci.com/gh/tarka/xcp)

### Features

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

## Testing

`libxcp` itself doesn't have many tests; the top-level `xcp` application however
has many tests, including fuzzed stress-tests. This should be considered the
test suite for now.
