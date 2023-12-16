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
    use std::{process::Command, fs::File};
    use libfs::map_extents;
    use test_case::test_case;

    use crate::util::*;

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    #[cfg_attr(feature = "test_no_reflink", ignore = "No FS support")]
    fn file_copy_reflink_always(drv: &str) {
        let dir = tempdir().unwrap();
        let source_path = dir.path().join("source.txt");
        let dest_path = dir.path().join("dest.txt");
        let text = "This is a test file.";

        create_file(&source_path, text).unwrap();

        let out = run(&[
            "--driver", drv,
            "--reflink=always",
            source_path.to_str().unwrap(),
            dest_path.to_str().unwrap(),
        ])
            .unwrap();

        // Should always work on CoW FS
        assert!(out.status.success());
        assert!(file_contains(&dest_path, text).unwrap());
        assert!(files_match(&source_path, &dest_path));

        let infd = File::open(&source_path).unwrap();
        let outfd = File::open(&dest_path).unwrap();

        let inext = map_extents(&infd).unwrap().unwrap();
        let outext = map_extents(&outfd).unwrap().unwrap();
        for (i, o) in inext.iter().zip(outext.iter()) {
            assert_eq!(i.start, o.start);
            assert_eq!(i.end, o.end);
            assert_eq!(i.shared, o.shared);
        }
    }

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
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

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
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

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
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

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
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

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_empty_sparse(drv: &str) {
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
    #[cfg_attr(not(feature = "test_run_expensive"), ignore = "Stress test")]
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
