# xcp: An extended cp

`xcp` is a (partial) clone of the Unix `cp` command. It is not intended as a
full replacement, but as a companion utility with some more user-friendly
feedback and some optimisations that make sense under certain tasks (see
below).

*Warning*: `xcp` is currently pre-alpha level software and almost certainly contains
bugs and unexpected or inconsistent behaviour. It probably shouldn't be used for
anything critical yet.

*Note*: `xcp` currently targets the [Rust 2018
edition](https://rust-lang-nursery.github.io/edition-guide/rust-2018/index.html). This
is due for release December 2018. You will need a recent beta of the Rust
toolchain to compile this code; if you are using `rustup` it will install it
automatically.

## Features and Anti-Features

### Features

* Displays a progress-bar, both for directory and single file copies. This can
  be disabled with `--no-progress`.
* Uses the Linux `copy_file_range` call to copy files. This is the most
  efficient method of file-copying under Linux; in particular it is
  filesystem-aware, and can massively speed-up copies on network mounts by
  performing the copy operations server-side. However, see Anti-Features below.
* Optionally understands `.gitignore` files to limit the copied directories.
* Optimised for 'modern' systems (i.e. multiple cores, copious RAM, and
  solid-state disks, especially ones connected into the main system bus,
  e.g. M.2).
  
### (Possible) future features

* Optional aggressive parallelism for systems with parallel IO. Quick
  experiments on a modern laptop suggest there may be benefits to parallel
  copies on NVRAM disks, this is obviously highly system-dependent.

### Anti-Features

* Currenly only supports Linux, specifically kernels 4.5 and onwards. Other
  Unix-like OS's may be added later.
* Reportedly `copy_file_range` does not understand sparse files, and will expand
  any 'holes' on disk. One common use of sparse files is virtual disks
  (e.g. from Virtualbox). Better sparse-file handling may be added later.
* Assumes a 'modern' system with lots of RAM and fast, solid-state disks. In
  particular it is likely to thrash on spinning disks as it attempts to gather
  metadata and perform copies at the same time.
* Currently missing a lot of `cp`'s features and flags, although these could be
  added.

## Performance

Benchmarks are mostly meaningless, but to check we're not introducing _too_ much
overhead for local copies, the following are results from a laptop with an NVMe
disk and in single-user mode. The target copy directory is a git checkout of the
Firefox codebase, having been recently gc'd (i.e. a single 4.1GB pack
file). `fstrim -va` is run before each test run.

### Local copy

* Single 4.1GB file copy, with the kernel cache dropped each run:
** `cp`: ~6.2s
** `xcp`: ~4.2s
* Single 4.1GB file copy, warmed cache (3 runs each):
** `cp`: ~1.85s
** `xcp`: ~1.7x
* Directory copy, kernel cache dropped each run:
** `cp`: ~48s
** `xcp`: ~56x
* Directory copy, warmed cache (3 runs each):
** `cp`: ~6.9s
** `xcp`: ~7.4s

To give an idea of how difficult meaningful benchmarks are, the reason `fstrim`
is run each time is that without it there is huget

### NFS copy

`xcp` uses `copy_file_range`, which is filesystem aware. On NFSv4 this will result
in the copy occuring server-side rather than tranferring across the network. For
large files this can be a significant win:

* Single 4.1GB file on NFSv4 mount
** `cp`: 378s
** `xcp`: ~37s
