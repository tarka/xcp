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
    use std::{process::Command, fs::{File, OpenOptions}, io::SeekFrom};
    use std::io::{Seek, Write};
    use libfs::{map_extents, sync};
    use test_case::test_case;

    use crate::util::*;

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    #[cfg_attr(feature = "test_no_reflink", ignore = "No FS support")]
    fn file_copy_reflink_always(drv: &str) {
        let dir = tempdir_rel().unwrap();
        let source_path = dir.path().join("source.bin");
        let dest_path = dir.path().join("dest.bin");
        let size = 128 * 1024;

        {
            let mut infd = File::create(&source_path).unwrap();
            let data = rand_data(size);
            infd.write_all(&data).unwrap();
        }

        {
            let infd = File::open(&source_path).unwrap();
            let inext = map_extents(&infd).unwrap().unwrap();
            // Single file, extent not shared.
            assert!(!inext[0].shared);
        }

        let out = run(&[
            "--driver", drv,
            "--reflink=always",
            source_path.to_str().unwrap(),
            dest_path.to_str().unwrap(),
        ])
            .unwrap();

        // Should always work on CoW FS
        assert!(out.status.success());
        assert!(files_match(&source_path, &dest_path));

        {
            let infd = File::open(&source_path).unwrap();
            let outfd = File::open(&dest_path).unwrap();
            // Extents should be shared.
            let inext = map_extents(&infd).unwrap().unwrap();
            let outext = map_extents(&outfd).unwrap().unwrap();
            assert!(inext[0].shared);
            assert!(outext[0].shared);
        }

        {
            let mut outfd = OpenOptions::new()
                .create(false)
                .write(true)
                .read(true)
                .open(&dest_path).unwrap();
            outfd.seek(SeekFrom::Start(0)).unwrap();
            let data = rand_data(size);
            outfd.write_all(&data).unwrap();
            // brtfs at least seems to need this to force CoW and
            // de-share the extents.
            sync(&outfd).unwrap();
        }

        {
            let infd = File::open(&source_path).unwrap();
            let outfd = File::open(&dest_path).unwrap();
            // First extent should now be un-shared.
            let inext = map_extents(&infd).unwrap().unwrap();
            let outext = map_extents(&outfd).unwrap().unwrap();
            assert!(!inext[0].shared);
            assert!(!outext[0].shared);
        }

    }

    #[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
    #[test_case("parfile"; "Test with parallel file driver")]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_sparse(drv: &str) {
        use std::fs::read;

        let dir = tempdir_rel().unwrap();
        let from = dir.path().join("sparse.bin");
        let to = dir.path().join("target.bin");

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

        let dir = tempdir_rel().unwrap();
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

        let dir = tempdir_rel().unwrap();
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

        let dir = tempdir_rel().unwrap();
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

        let dir = tempdir_rel().unwrap();
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
        // Spam some output to keep CI from timing-out (hopefully).
        println!("Generating file tree...");
        let src = gen_global_filetree(false).unwrap();

        let dir = tempdir_rel().unwrap();
        let dest = dir.path().join("target");

        println!("Running copy...");
        let out = run(&[
            "--driver", drv,
            "-r",
            "--no-progress",
            src.to_str().unwrap(),
            dest.to_str().unwrap(),
        ]).unwrap();
        assert!(out.status.success());

        println!("Compare trees...");
        compare_trees(&src, &dest).unwrap();
    }
}
