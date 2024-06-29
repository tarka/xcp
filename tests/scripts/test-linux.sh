#!/usr/bin/bash

set -euo pipefail

# chdir to source root
cd "$(dirname "$0")"/../..

# get the name of the filesystem that contains the source code
fs=$(df --output=fstype . | tail -n 1)

# list features supported by all filesystems
features=(use_linux "$@")

# disable tests that will not work on this filesystem
case "$fs" in
xfs | btrfs | bcachefs) ;;

ext4)
  features+=(
    test_no_reflink
  )
  ;;

ext[23])
  features+=(
    test_no_extents
    test_no_reflink
  )
  ;;

f2fs)
  features+=(
    test_no_reflink
  )
  ;;

fuseblk)
  echo >&2 "WARNING: assuming ntfs"
  features+=(
    test_no_acl
    test_no_extents
    test_no_reflink
    test_no_sparse
    test_no_perms
  )
  ;;

vfat)
  features+=(
    test_no_acl
    test_no_extents
    test_no_reflink
    test_no_sockets
    test_no_sparse
    test_no_symlinks
    test_no_xattr
    test_no_perms
  )
  ;;

tmpfs)
  features+=(
    test_no_extents
    test_no_reflink
    test_no_sparse
  )
  ;;

zfs)
  features+=(
    test_no_acl
    test_no_extents
    test_no_reflink
    test_no_sparse
  )
  ;;

*)
  echo >&2 "WARNING: unknown filesystem $fs, advanced FS tests disabled."
  features+=(
    test_no_acl
    test_no_extents
    test_no_reflink
    test_no_sparse
  )
  ;;
esac

echo >&2 "found filesystem $fs, using flags ${features[*]}"

cargo test --workspace --release --locked --features "$(
  export IFS=,
  echo "${features[*]}"
)"
