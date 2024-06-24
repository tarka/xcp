#!/bin/bash

echo "KERNEL"
uname -a
ls -al /boot/
grep BCACHEFS /boot/config-$(uname -r)

sudo apt update

sudo apt install -y zfsutils-linux xfsprogs \
     btrfs-progs ntfs-3g dosfstools bcachefs-tools

for fs in "$@"; do
    root=/fs/$fs
    img=$root.img

    echo >&2 "==== creating $fs in $root ===="

    sudo mkdir --parents $root
    sudo fallocate --length 2.5G $img

    case $fs in
        zfs)  sudo zpool create -m $root test $img ;;
        ntfs) sudo mkfs.ntfs --fast --force $img   ;;
        *)    sudo mkfs.$fs $img                   ;;
    esac

    # zfs gets automounted
    if [[ $fs != zfs ]]; then
        if [[ $fs = fat ]]; then
            sudo mount -o uid=$(id -u) $img $root
        else
            sudo mount $img $root
        fi
    fi

    # fat mount point cannot be chowned
    # and is handled by the uid= option above
    if [[ $fs != fat ]]; then
        sudo chown $USER $root
    fi

    git clone . $root/src
done

findmnt --real
