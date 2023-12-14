/*
 * Copyright © 2018, Steve Smith <tarkasteve@gmail.com>
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

mod util;

#[cfg(all(target_os = "linux", feature = "use_linux"))]
mod test {
    use std::{process::Command, os::unix::net::UnixListener, fs::create_dir_all};
    use test_case::test_case;

    use crate::util::*;

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    fn test_socket_file(drv: &str) {
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.sock");
        let to = dir.path().join("to.sock");

        let _sock = UnixListener::bind(&from).unwrap();
        let ftype = from.metadata().unwrap().file_type();
        assert!(!ftype.is_file() && !ftype.is_dir() && !ftype.is_symlink());

        let out = run(&[
            "--driver", drv,
            from.to_str().unwrap(),
            to.to_str().unwrap(),
        ]).unwrap();
        assert!(out.status.success());

        assert!(to.exists());
        let ftype = to.metadata().unwrap().file_type();
        assert!(!ftype.is_file() && !ftype.is_dir() && !ftype.is_symlink());
    }

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    fn test_sockets_dir(drv: &str) {

        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("fromdir");
        create_dir_all(&src_dir).unwrap();

        let from = src_dir.join("from.sock");
        let _sock = UnixListener::bind(&from).unwrap();
        let ftype = from.metadata().unwrap().file_type();
        assert!(!ftype.is_file() && !ftype.is_dir() && !ftype.is_symlink());

        let to_dir = dir.path().join("todir");
        let to = to_dir.join("from.sock");

        let out = run(&[
            "--driver", drv,
            "-r",
            src_dir.to_str().unwrap(),
            to_dir.to_str().unwrap(),
        ]).unwrap();
        assert!(out.status.success());

        assert!(to.exists());
        let ftype = to.metadata().unwrap().file_type();
        assert!(!ftype.is_file() && !ftype.is_dir() && !ftype.is_symlink());
    }

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    fn test_sparse(drv: &str) {
        if !fs_supports_sparse() {
            return
        }
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

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    fn test_sparse_leading_gap(drv: &str) {
        if !fs_supports_sparse() {
            return
        }
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

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    fn test_sparse_trailng_gap(drv: &str) {
        if !fs_supports_sparse() {
            return
        }
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

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    fn test_sparse_single_overwrite(drv: &str) {
        if !fs_supports_sparse() {
            return
        }
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

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    fn test_empty_sparse(drv: &str) {
        if !fs_supports_sparse() {
            return
        }
        use std::fs::read;

        let dir = tempdir().unwrap();
        let from = dir.path().join("sparse.bin");
        let to = dir.path().join("target.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", from.to_str().unwrap()])
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


    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
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
            "--driver", drv,
            "-r",
            src.to_str().unwrap(),
            dest.to_str().unwrap(),
        ]).unwrap();
        assert!(out.status.success());

        println!("Compare trees...");
        compare_trees(&src, &dest).unwrap();
    }
}
