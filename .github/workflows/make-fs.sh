#!/bin/bash

fs=$1

sudo apt update
sudo apt install -y xfsprogs btrfs-progs ntfs-3g dosfstools
if [[ $fs = zfs ]]; then
    sudo apt install -y zfsutils-linux
fi

root=/fs/$fs
img=$root.img

echo >&2 "==== creating $fs in $root ===="

sudo mkdir --parents $root
sudo fallocate --length 32G $img

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

findmnt --real
