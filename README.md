# `xcp`: An extended `cp`

`xcp` is a (partial) clone of the Unix `cp` command. It is not intended as a
full replacement, but as a companion utility with some more user-friendly
feedback and some optimisations that make sense under certain tasks (see
below).

Note: `xcp` is currently pre-alpha level software and almost certainly contains
bugs and unexpected or inconsistent behaviour. It probably shouldn't be used for
anything critical yet.

## Features and Anti-Features

### Features

* Displays a progress-bar, both for directory and single file copies. This can
  be disabled with $FIXME.
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

NFS - 4.1G Single file
  cp 6.18
  xcp 37

Local dir - progress
  cp 14-28
  xcp 24-35

Local dir - cache-flush, progress
  cp ~31
  xcp ~39

Local file 4.1G - progress
  cp 2-6s
  xcp 2-4s

