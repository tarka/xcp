
use std::fs::{File, OpenOptions, read};
use std::io::Write;
use std::ops::Range;
use tempfile::{tempdir_in, TempDir};

use crate::linux::{FiemapReq, SeekOff, fiemap, lseek};
use crate::{allocate_file, copy_file_bytes, copy_file_offset, copy_node, copy_permissions, map_extents, probably_sparse, reflink};
use std::env::current_dir;
use std::io::{self, Seek};
use std::iter;
use std::os::unix::net::UnixListener;
use std::process::Command;
use linux_raw_sys::ioctl::{FIEMAP_EXTENT_SHARED};
use rustix::fs::{FileTypeExt, SeekFrom, copy_file_range};

use crate::errors::Result;
use crate::{Extent, merge_extents};
use crate::common::{copy_bytes_uspace, copy_range_uspace};

impl From<Range<u64>> for Extent {
    fn from(r: Range<u64>) -> Self {
        Extent {
            start: r.start,
            end: r.end,
            shared: false,
        }
    }
}

#[test]
fn test_copy_bytes_uspace_large() {
    let dir = tempdir().unwrap();
    let from = dir.path().join("from.bin");
    let to = dir.path().join("to.bin");
    let size = 128 * 1024;
    let data = "X".repeat(size);

    {
        let mut fd: File = File::create(&from).unwrap();
        write!(fd, "{data}").unwrap();
    }

    {
        let infd = File::open(&from).unwrap();
        let outfd = File::create(&to).unwrap();
        let written = copy_bytes_uspace(&infd, &outfd, size).unwrap();

        assert_eq!(written, size);
    }

    assert_eq!(from.metadata().unwrap().len(), to.metadata().unwrap().len());

    {
        let from_data = read(&from).unwrap();
        let to_data = read(&to).unwrap();
        assert_eq!(from_data, to_data);
    }
}

#[test]
fn test_copy_range_uspace_large() {
    let dir = tempdir().unwrap();
    let from = dir.path().join("from.bin");
    let to = dir.path().join("to.bin");
    let size = 128 * 1024;
    let data = "X".repeat(size);

    {
        let mut fd: File = File::create(&from).unwrap();
        write!(fd, "{data}").unwrap();
    }

    {
        let infd = File::open(&from).unwrap();
        let outfd = File::create(&to).unwrap();

        let blocksize = size / 4;
        let mut written = 0;

        for off in (0..4).rev() {
            written += copy_range_uspace(&infd, &outfd, blocksize, blocksize * off).unwrap();
        }

        assert_eq!(written, size);
    }

    assert_eq!(from.metadata().unwrap().len(), to.metadata().unwrap().len());

    {
        let from_data = read(&from).unwrap();
        let to_data = read(&to).unwrap();
        assert_eq!(from_data, to_data);
    }
}

#[test]
fn test_extent_merge() -> Result<()> {
    assert_eq!(merge_extents(vec!())?, vec!());
    assert_eq!(merge_extents(
        vec!((0..1).into()))?,
               vec!((0..1).into()));

    assert_eq!(merge_extents(
        vec!((0..1).into(),
             (10..20).into()))?,
               vec!((0..1).into(),
                    (10..20).into()));
    assert_eq!(merge_extents(
        vec!((0..10).into(),
             (11..20).into()))?,
               vec!((0..20).into()));
    assert_eq!(
        merge_extents(
            vec!((0..5).into(),
                 (11..20).into(),
                 (21..30).into(),
                 (40..50).into()))?,
        vec!((0..5).into(),
             (11..30).into(),
             (40..50).into())
    );
    assert_eq!(
        merge_extents(vec!((0..5).into(),
                           (11..20).into(),
                           (21..30).into(),
                           (40..50).into(),
                           (51..60).into()))?,
        vec!((0..5).into(),
             (11..30).into(),
             (40..60).into())
    );
    assert_eq!(
        merge_extents(
            vec!((0..10).into(),
                 (11..20).into(),
                 (21..30).into(),
                 (31..50).into(),
                 (51..60).into()))?,
        vec!((0..60).into())
    );
    Ok(())
}


#[test]
fn test_copy_file() -> Result<()> {
    let dir = tempdir()?;
    let from = dir.path().join("file.bin");
    let len = 32 * 1024 * 1024;

    {
        let mut fd = File::create(&from)?;
        let data = "X".repeat(len);
        write!(fd, "{data}").unwrap();
    }

    assert_eq!(len, from.metadata()?.len() as usize);

    let to = dir.path().join("sparse.copy.bin");
    crate::copy_file(&from, &to)?;

    assert_eq!(len, to.metadata()?.len() as usize);

    Ok(())
}


    fn tempdir() -> Result<TempDir> {
        // Force into local dir as /tmp might be tmpfs, which doesn't
        // support all VFS options (notably fiemap).
        Ok(tempdir_in(current_dir()?.join("../target"))?)
    }

    #[test]
    #[cfg_attr(feature = "test_no_reflink", ignore = "No FS support")]
    fn test_reflink() -> Result<()> {
        let dir = tempdir()?;
        let from = dir.path().join("file.bin");
        let to = dir.path().join("copy.bin");
        let size = 128 * 1024;

        {
            let mut fd: File = File::create(&from)?;
            let data = "X".repeat(size);
            write!(fd, "{data}")?;
        }

        let from_fd = File::open(from)?;
        let to_fd = File::create(to)?;

        {
            let mut from_map = FiemapReq::new();
            assert!(fiemap(&from_fd, &mut from_map)?);
            assert!(from_map.fm_mapped_extents > 0);
            // Un-refed file, no shared extents
            assert!(from_map.fm_extents[0].fe_flags & FIEMAP_EXTENT_SHARED == 0);
        }

        let worked = reflink(&from_fd, &to_fd)?;
        assert!(worked);

        {
            let mut from_map = FiemapReq::new();
            assert!(fiemap(&from_fd, &mut from_map)?);
            assert!(from_map.fm_mapped_extents > 0);

            let mut to_map = FiemapReq::new();
            assert!(fiemap(&to_fd, &mut to_map)?);
            assert!(to_map.fm_mapped_extents > 0);

            // Now both have shared extents
            assert_eq!(from_map.fm_mapped_extents, to_map.fm_mapped_extents);
            assert!(from_map.fm_extents[0].fe_flags & FIEMAP_EXTENT_SHARED != 0);
            assert!(to_map.fm_extents[0].fe_flags & FIEMAP_EXTENT_SHARED != 0);
        }

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_sparse_detection_small_data() -> Result<()> {
        assert!(!probably_sparse(&File::open("Cargo.toml")?)?);

        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        {
            let fd = File::open(&file)?;
            assert!(probably_sparse(&fd)?);
        }
        {
            let mut fd = OpenOptions::new().write(true).append(false).open(&file)?;
            write!(fd, "test")?;
            assert!(probably_sparse(&fd)?);
        }

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_sparse_detection_half() -> Result<()> {
        assert!(!probably_sparse(&File::open("Cargo.toml")?)?);

        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());
        {
            let mut fd = OpenOptions::new().write(true).append(false).open(&file)?;
            let s = "x".repeat(512*1024);
            fd.write_all(s.as_bytes())?;
            assert!(probably_sparse(&fd)?);
        }

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_copy_bytes_sparse() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";

        {
            let mut fd = File::create(&from)?;
            write!(fd, "{data}")?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            copy_file_bytes(&infd, &outfd, data.len() as u64)?;
        }

        assert!(probably_sparse(&File::open(file)?)?);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_sparse_copy_middle() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";

        {
            let mut fd = File::create(&from)?;
            write!(fd, "{data}")?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let offset = 512 * 1024;
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let mut off_in = 0;
            let mut off_out = offset as u64;
            let copied = copy_file_range(
                &infd,
                Some(&mut off_in),
                &outfd,
                Some(&mut off_out),
                data.len(),
            )?;
            assert_eq!(copied as usize, data.len());
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let bytes = read(&file)?;

        assert!(bytes.len() == 1024 * 1024);
        assert!(bytes[offset] == b't');
        assert!(bytes[offset + 1] == b'e');
        assert!(bytes[offset + 2] == b's');
        assert!(bytes[offset + 3] == b't');
        assert!(bytes[offset + data.len()] == 0);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_copy_range_middle() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";
        let offset: usize = 512 * 1024;

        {
            let mut fd = File::create(&from)?;
            fd.seek(io::SeekFrom::Start(offset as u64))?;
            write!(fd, "{data}")?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let copied =
                copy_file_offset(&infd, &outfd, data.len() as u64, offset as i64)?;
            assert_eq!(copied as usize, data.len());
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let bytes = read(&file)?;
        assert_eq!(bytes.len(), 1024 * 1024);
        assert_eq!(bytes[offset], b't');
        assert_eq!(bytes[offset + 1], b'e');
        assert_eq!(bytes[offset + 2], b's');
        assert_eq!(bytes[offset + 3], b't');
        assert_eq!(bytes[offset + data.len()], 0);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_lseek_data() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";
        let offset = 512 * 1024;

        {
            let mut fd = File::create(&from)?;
            write!(fd, "{data}")?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let mut off_in = 0;
            let mut off_out = offset;
            let copied = copy_file_range(
                &infd,
                Some(&mut off_in),
                &outfd,
                Some(&mut off_out),
                data.len(),
            )?;
            assert_eq!(copied as usize, data.len());
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let off = lseek(&File::open(&file)?, SeekFrom::Data(0))?;
        assert_eq!(off, SeekOff::Offset(offset));

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_sparse_rust_seek() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let data = "c00lc0d3";

        {
            let mut fd = File::create(&file)?;
            write!(fd, "{data}")?;

            fd.seek(io::SeekFrom::Start(1024 * 4096))?;
            write!(fd, "{data}")?;

            fd.seek(io::SeekFrom::Start(4096 * 4096 - data.len() as u64))?;
            write!(fd, "{data}")?;
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let bytes = read(&file)?;
        assert!(bytes.len() == 4096 * 4096);

        let offset = 1024 * 4096;
        assert!(bytes[offset] == b'c');
        assert!(bytes[offset + 1] == b'0');
        assert!(bytes[offset + 2] == b'0');
        assert!(bytes[offset + 3] == b'l');
        assert!(bytes[offset + data.len()] == 0);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_lseek_no_data() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());
        assert!(probably_sparse(&File::open(&file)?)?);

        let fd = File::open(&file)?;
        let off = lseek(&fd, SeekFrom::Data(0))?;
        assert!(off == SeekOff::EOF);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_allocate_file_is_sparse() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let len = 32 * 1024 * 1024;

        {
            let fd = File::create(&file)?;
            allocate_file(&fd, len)?;
        }

        assert_eq!(len, file.metadata()?.len());
        assert!(probably_sparse(&File::open(&file)?)?);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_empty_extent() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let fd = File::open(file)?;

        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();
        assert_eq!(extents.len(), 0);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_extent_fetch() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";

        {
            let mut fd = File::create(&from)?;
            write!(fd, "{data}")?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let offset = 512 * 1024;
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let mut off_in = 0;
            let mut off_out = offset;
            let copied = copy_file_range(
                &infd,
                Some(&mut off_in),
                &outfd,
                Some(&mut off_out),
                data.len(),
            )?;
            assert_eq!(copied as usize, data.len());
        }

        let fd = File::open(file)?;

        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();
        assert_eq!(extents.len(), 1);
        assert_eq!(extents[0].start, offset);
        assert_eq!(extents[0].end, offset + 4 * 1024); // FIXME: Assume 4k blocks
        assert!(!extents[0].shared);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_extent_fetch_many() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let fsize = 1024 * 1024;
        // FIXME: Assumes 4k blocks
        let bsize = 4 * 1024;
        let block = iter::repeat_n(0xff_u8, bsize).collect::<Vec<u8>>();

        let mut fd = OpenOptions::new().write(true).append(false).open(&file)?;
        // Skip every-other block
        for off in (0..fsize).step_by(bsize * 2) {
            lseek(&fd, SeekFrom::Start(off))?;
            fd.write_all(block.as_slice())?;
        }

        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();
        assert_eq!(extents.len(), fsize as usize / bsize / 2);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_extent_not_sparse() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("file.bin");
        let size = 128 * 1024;

        {
            let mut fd: File = File::create(&file)?;
            let data = "X".repeat(size);
            write!(fd, "{data}")?;
        }

        let fd = File::open(file)?;
        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();

        assert_eq!(1, extents.len());
        assert_eq!(0u64, extents[0].start);
        assert_eq!(size as u64, extents[0].end);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_extent_unsupported_fs() -> Result<()> {
        let file = "/proc/cpuinfo";
        let fd = File::open(file)?;
        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_none());

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_copy_file_sparse() -> Result<()> {
        let dir = tempdir()?;
        let from = dir.path().join("sparse.bin");
        let len = 32 * 1024 * 1024;

        {
            let fd = File::create(&from)?;
            allocate_file(&fd, len)?;
        }

        assert_eq!(len, from.metadata()?.len());
        assert!(probably_sparse(&File::open(&from)?)?);

        let to = dir.path().join("sparse.copy.bin");
        crate::copy_file(&from, &to)?;

        assert_eq!(len, to.metadata()?.len());
        assert!(probably_sparse(&File::open(&to)?)?);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sockets", ignore = "No FS support")]
    fn test_copy_socket() {
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.sock");
        let to = dir.path().join("to.sock");

        let _sock = UnixListener::bind(&from).unwrap();
        assert!(from.metadata().unwrap().file_type().is_socket());

        copy_node(&from, &to).unwrap();

        assert!(to.exists());
        assert!(to.metadata().unwrap().file_type().is_socket());
    }


    #[test]
    #[cfg_attr(feature = "test_no_acl", ignore = "No FS support")]
    fn test_copy_acl() -> Result<()> {
        use exacl::{getfacl, AclEntry, Perm, setfacl};

        let dir = tempdir()?;
        let from = dir.path().join("file.bin");
        let to = dir.path().join("copy.bin");
        let data = "X".repeat(1024);

        {
            let mut fd: File = File::create(&from)?;
            write!(fd, "{data}")?;

            let mut fd: File = File::create(&to)?;
            write!(fd, "{data}")?;
        }

        let acl = AclEntry::allow_user("mail", Perm::READ, None);

        let mut from_acl = getfacl(&from, None)?;
        from_acl.push(acl.clone());
        setfacl(&[&from], &from_acl, None)?;

        {
            let from_fd: File = File::open(&from)?;
            let to_fd: File = File::open(&to)?;
            copy_permissions(&from_fd, &to_fd)?;
        }

        let to_acl = getfacl(&from, None)?;
        assert!(to_acl.contains(&acl));

        Ok(())
    }
