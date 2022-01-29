/*
 * Copyright © 2022, Steve Smith <tarkasteve@gmail.com>
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

use std::path::PathBuf;

use cfg_if::cfg_if;
cfg_if! {if #[cfg(any(target_os = "linux", target_os = "android"))] {

    use std::process::Command;
    use test_case::test_case;

    mod util;
    use crate::util::*;

    #[test_case("parfile"; "Test with parallel file driver")]
    #[test_case("parblock"; "Test with parallel block driver")]
    #[ignore] // Expensive so skip for local dev
    fn copy_generated_tree_sparse(drv: &str) {
        let dir = tempdir().unwrap();

        let src = dir.path().join("generated");
        let dest = dir.path().join("target");

        // Spam some output to keep CI from timing-out (hopefully).
        println!("Generating file tree...");
        gen_filetree(&src, 0, true).unwrap();

        println!("Running copy...");
        let out = run(&[
            "--driver",
            drv,
            "-r",
            src.to_str().unwrap(),
            dest.to_str().unwrap(),
        ]).unwrap();
        assert!(out.status.success());

        println!("Compare trees...");
        compare_trees(&src, &dest).unwrap();
    }

    #[test_case("parfile"; "Test with parallel file driver")]
    #[test_case("parblock"; "Test with parallel block driver")]
    fn test_sparse(drv: &str) {
        use std::fs::read;

        let dir = tempdir_rel().unwrap();
        let from = dir.join("sparse.bin");
        let to = dir.join("target.bin");

        let slen = create_sparse(&from, 0, 0).unwrap();
        assert_eq!(slen, from.metadata().unwrap().len());
        assert!(probably_sparse(&from).unwrap());

        let out = run(&[
            "--driver",
            drv,
            from.to_str().unwrap(),
            to.to_str().unwrap(),
        ]).unwrap();
        assert!(out.status.success());

        assert!(probably_sparse(&to).unwrap());

        assert_eq!(quickstat(&from).unwrap(), quickstat(&to).unwrap());

        let from_data = read(&from).unwrap();
        let to_data = read(&to).unwrap();
        assert_eq!(from_data, to_data);
    }

    #[test_case("parfile"; "Test with parallel file driver")]
    #[test_case("parblock"; "Test with parallel block driver")]
    fn test_sparse_leading_gap(drv: &str) {
        use std::fs::read;

        let dir = tempdir().unwrap();
        let from = dir.path().join("sparse.bin");
        let to = dir.path().join("target.bin");

        let slen = create_sparse(&from, 1024, 0).unwrap();
        assert_eq!(slen, from.metadata().unwrap().len());
        assert!(probably_sparse(&from).unwrap());

        let out = run(&[
            "--driver",
            drv,
            from.to_str().unwrap(),
            to.to_str().unwrap(),
        ]).unwrap();

        assert!(out.status.success());
        assert!(probably_sparse(&to).unwrap());
        assert_eq!(quickstat(&from).unwrap(), quickstat(&to).unwrap());

        let from_data = read(&from).unwrap();
        let to_data = read(&to).unwrap();
        assert_eq!(from_data, to_data);
    }

    #[test_case("parfile"; "Test with parallel file driver")]
    #[test_case("parblock"; "Test with parallel block driver")]
    fn test_sparse_trailng_gap(drv: &str) {
        use std::fs::read;

        let dir = tempdir().unwrap();
        let from = dir.path().join("sparse.bin");
        let to = dir.path().join("target.bin");

        let slen = create_sparse(&from, 1024, 1024).unwrap();
        assert_eq!(slen, from.metadata().unwrap().len());
        assert!(probably_sparse(&from).unwrap());

        let out = run(&[
            "--driver",
            drv,
            from.to_str().unwrap(),
            to.to_str().unwrap(),
        ]).unwrap();
        assert!(out.status.success());

        assert!(probably_sparse(&to).unwrap());
        assert_eq!(quickstat(&from).unwrap(), quickstat(&to).unwrap());

        let from_data = read(&from).unwrap();
        let to_data = read(&to).unwrap();
        assert_eq!(from_data, to_data);
    }

    #[test_case("parfile"; "Test with parallel file driver")]
    #[test_case("parblock"; "Test with parallel block driver")]
    fn test_sparse_single_overwrite(drv: &str) {
        use std::fs::read;

        let dir = tempdir().unwrap();
        let from = dir.path().join("sparse.bin");
        let to = dir.path().join("target.bin");

        let slen = create_sparse(&from, 1024, 1024).unwrap();
        create_file(&to, "").unwrap();
        assert_eq!(slen, from.metadata().unwrap().len());
        assert!(probably_sparse(&from).unwrap());

        let out = run(&[
            "--driver",
            drv,
            from.to_str().unwrap(),
            to.to_str().unwrap(),
        ]).unwrap();
        assert!(out.status.success());
        assert!(probably_sparse(&to).unwrap());
        assert_eq!(quickstat(&from).unwrap(), quickstat(&to).unwrap());

        let from_data = read(&from).unwrap();
        let to_data = read(&to).unwrap();
        assert_eq!(from_data, to_data);
    }

    #[test_case("parfile"; "Test with parallel file driver")]
    #[test_case("parblock"; "Test with parallel block driver")]
    fn test_empty_sparse(drv: &str) {
        use std::fs::read;

        let dir = tempdir().unwrap();
        let from = dir.path().join("sparse.bin");
        let to = dir.path().join("target.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", from.to_str().unwrap()])
            .output().unwrap();
        assert!(out.status.success());
        assert_eq!(from.metadata().unwrap().len(), 1024 * 1024);

        let out = run(&[
            "--driver",
            drv,
            from.to_str().unwrap(),
            to.to_str().unwrap(),
        ]).unwrap();
        assert!(out.status.success());
        assert_eq!(to.metadata().unwrap().len(), 1024 * 1024);

        assert!(probably_sparse(&to).unwrap());
        assert_eq!(quickstat(&from).unwrap(), quickstat(&to).unwrap());

        let from_data = read(&from).unwrap();
        let to_data = read(&to).unwrap();
        assert_eq!(from_data, to_data);
    }

    #[test_case("parfile"; "Test with parallel file driver")]
    #[test_case("parblock"; "Test with parallel block driver")]
    fn copy_procfs_file(drv: &str) {
        // Copying files on virtual filesystems (e.g. proc) with
        // copy_file_range() has been known to have issues; see
        // https://lwn.net/Articles/846403/ for details.

        let dir = tempdir().unwrap();
        let source_path = PathBuf::from("/proc/cpuinfo");
        let dest_path = dir.path().join("dest.txt");
        let out = run(&[
            "--driver",
            drv,
            source_path.to_str().unwrap(),
            dest_path.to_str().unwrap(),
        ]).unwrap();

        assert!(out.status.success());
        let (size, _blocks, _blksize) = quickstat(&dest_path).unwrap();
        assert!(size > 0);
    }

    #[test_case("parfile"; "Test with parallel file driver")]
    #[test_case("parblock"; "Test with parallel block driver")]
    fn copy_sysfs_file(drv: &str) {
        // Copying files on virtual filesystems (e.g. proc) with
        // copy_file_range() has been known to have issues; see
        // https://lwn.net/Articles/846403/ for details.

        let dir = tempdir().unwrap();
        let source_path = PathBuf::from("/sys/kernel/vmcoreinfo");
        let dest_path = dir.path().join("dest.txt");
        let out = run(&[
            "--driver",
            drv,
            source_path.to_str().unwrap(),
            dest_path.to_str().unwrap(),
        ]).unwrap();

        assert!(out.status.success());
        let (size, _blocks, _blksize) = quickstat(&dest_path).unwrap();
        assert!(size > 0);
    }

}}
