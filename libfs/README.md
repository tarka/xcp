# libfs: Advanced file and fs operations

`libfs` is a library of file and filesystem operations that is supplementary to
[std::fs](https://doc.rust-lang.org/std/fs/). Current features:

* High and mid-level functions for creating and copying sparse files.
* Copying will use Linux
  [copy_file_range](https://man7.org/linux/man-pages/man2/copy_file_range.2.html)
  where possible, with fall-back to userspace.
* Scanning and merging extent information on filesystems that support it.
* File permission copying, including
  [xattrs](https://man7.org/linux/man-pages/man7/xattr.7.html).

Some of the features are Linux specific, but most have fall-back alternative
implementations for other Unix-like OSs. Further support is todo.

`libfs` is part of the [xcp](https://crates.io/crates/xcp) project.

[![Crates.io](https://img.shields.io/crates/v/xcp.svg?colorA=777777)](https://crates.io/crates/libfs)
![Github Actions](https://github.com/tarka/xcp/actions/workflows/tests.yml/badge.svg)
[![CircleCI](https://circleci.com/gh/tarka/xcp.svg?style=shield)](https://circleci.com/gh/tarka/xcp)
