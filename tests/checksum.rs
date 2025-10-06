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

use std::fs::{File, create_dir_all};
use std::io::Write;
use test_case::test_case;

mod util;
use crate::util::*;

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_basic_copy(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source = dir.path().join("source.bin");
    let dest = dir.path().join("dest.bin");

    let data = rand_data(1024 * 1024);
    File::create(&source).unwrap().write_all(&data).unwrap();

    let out = run(&[
        "--driver", drv,
        "--verify-checksum",
        source.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success(), "Copy with checksum verification should succeed");
    assert!(dest.exists());
    assert!(files_match(&source, &dest));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_empty_file(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source = dir.path().join("empty.bin");
    let dest = dir.path().join("empty_copy.bin");

    File::create(&source).unwrap();

    let out = run(&[
        "--driver", drv,
        "--verify-checksum",
        source.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(dest.exists());
    assert_eq!(dest.metadata().unwrap().len(), 0);
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_small_file(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source = dir.path().join("small.txt");
    let dest = dir.path().join("small_copy.txt");

    create_file(&source, "Hello, World!").unwrap();

    let out = run(&[
        "--driver", drv,
        "--verify-checksum",
        source.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest, "Hello, World!").unwrap());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_large_file(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source = dir.path().join("large.bin");
    let dest = dir.path().join("large_copy.bin");

    let data = rand_data(10 * 1024 * 1024);
    File::create(&source).unwrap().write_all(&data).unwrap();

    let out = run(&[
        "--driver", drv,
        "--verify-checksum",
        source.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(files_match(&source, &dest));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_multiple_files(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest_dir = dir.path().join("dest");
    create_dir_all(&dest_dir).unwrap();

    let file1 = dir.path().join("file1.bin");
    let file2 = dir.path().join("file2.bin");
    let file3 = dir.path().join("file3.bin");

    File::create(&file1).unwrap().write_all(&rand_data(1024)).unwrap();
    File::create(&file2).unwrap().write_all(&rand_data(2048)).unwrap();
    File::create(&file3).unwrap().write_all(&rand_data(4096)).unwrap();

    let out = run(&[
        "--driver", drv,
        "--verify-checksum",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
        file3.to_str().unwrap(),
        dest_dir.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(files_match(&file1, &dest_dir.join("file1.bin")));
    assert!(files_match(&file2, &dest_dir.join("file2.bin")));
    assert!(files_match(&file3, &dest_dir.join("file3.bin")));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_directory_recursive(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_dir = dir.path().join("source");
    let dest_dir = dir.path().join("dest");

    create_dir_all(&source_dir).unwrap();
    create_dir_all(source_dir.join("subdir")).unwrap();

    create_file(&source_dir.join("file1.txt"), "content1").unwrap();
    create_file(&source_dir.join("subdir/file2.txt"), "content2").unwrap();

    let data = rand_data(512 * 1024);
    File::create(source_dir.join("binary.bin")).unwrap().write_all(&data).unwrap();

    let out = run(&[
        "--driver", drv,
        "-r",
        "--verify-checksum",
        "-T",
        source_dir.to_str().unwrap(),
        dest_dir.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(dest_dir.exists());
    assert!(files_match(&source_dir.join("file1.txt"), &dest_dir.join("file1.txt")));
    assert!(files_match(&source_dir.join("subdir/file2.txt"), &dest_dir.join("subdir/file2.txt")));
    assert!(files_match(&source_dir.join("binary.bin"), &dest_dir.join("binary.bin")));
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[cfg_attr(not(feature = "test_no_sparse"), cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver")))]
#[cfg_attr(not(feature = "test_no_sparse"), test_case("parfile"; "Test with parallel file driver"))]
fn checksum_sparse_file(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source = dir.path().join("sparse.bin");
    let dest = dir.path().join("sparse_copy.bin");

    let _size = create_sparse(&source, 0, 1024).unwrap();

    let out = run(&[
        "--driver", drv,
        "--verify-checksum",
        source.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(files_match(&source, &dest));
    assert!(probably_sparse(&dest).unwrap());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_block_sizes(drv: &str) {
    for block_size in ["64KB", "256KB", "1MB", "4MB"] {
        let dir = tempdir_rel().unwrap();
        let source = dir.path().join("source.bin");
        let dest = dir.path().join(format!("dest_{}.bin", block_size));

        let data = rand_data(2 * 1024 * 1024);
        File::create(&source).unwrap().write_all(&data).unwrap();

        let out = run(&[
            "--driver", drv,
            "--block-size", block_size,
            "--verify-checksum",
            source.to_str().unwrap(),
            dest.to_str().unwrap(),
        ]).unwrap();

        assert!(out.status.success(), "Failed with block size {}", block_size);
        assert!(files_match(&source, &dest));
    }
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_with_workers(drv: &str) {
    for workers in [1, 2, 4, 8] {
        let dir = tempdir_rel().unwrap();
        let source = dir.path().join("source.bin");
        let dest = dir.path().join(format!("dest_w{}.bin", workers));

        let data = rand_data(1024 * 1024);
        File::create(&source).unwrap().write_all(&data).unwrap();

        let out = run(&[
            "--driver", drv,
            "--workers", &workers.to_string(),
            "--verify-checksum",
            source.to_str().unwrap(),
            dest.to_str().unwrap(),
        ]).unwrap();

        assert!(out.status.success(), "Failed with {} workers", workers);
        assert!(files_match(&source, &dest));
    }
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_binary_patterns(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let patterns = [
        ("zeros", vec![0u8; 1024 * 1024]),
        ("ones", vec![0xFFu8; 1024 * 1024]),
        ("alternating", (0..1024*1024).map(|i| if i % 2 == 0 { 0xAA } else { 0x55 }).collect()),
    ];

    for (name, data) in patterns {
        let source = dir.path().join(format!("{}.bin", name));
        let dest = dir.path().join(format!("{}_copy.bin", name));

        File::create(&source).unwrap().write_all(&data).unwrap();

        let out = run(&[
            "--driver", drv,
            "--verify-checksum",
            source.to_str().unwrap(),
            dest.to_str().unwrap(),
        ]).unwrap();

        assert!(out.status.success(), "Failed for pattern: {}", name);
        assert!(files_match(&source, &dest));
    }
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_overwrite_existing(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source = dir.path().join("source.bin");
    let dest = dir.path().join("dest.bin");

    let data1 = rand_data(1024);
    let data2 = rand_data(2048);

    File::create(&dest).unwrap().write_all(&data1).unwrap();
    File::create(&source).unwrap().write_all(&data2).unwrap();

    let out = run(&[
        "--driver", drv,
        "--verify-checksum",
        source.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(files_match(&source, &dest));
    assert_eq!(dest.metadata().unwrap().len(), 2048);
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_with_fsync(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source = dir.path().join("source.bin");
    let dest = dir.path().join("dest.bin");

    let data = rand_data(512 * 1024);
    File::create(&source).unwrap().write_all(&data).unwrap();

    let out = run(&[
        "--driver", drv,
        "--verify-checksum",
        "--fsync",
        source.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(files_match(&source, &dest));
}

#[cfg(feature = "test_run_expensive")]
#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_random_tree(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_dir = dir.path().join("random_tree");
    let dest_dir = dir.path().join("random_tree_copy");

    gen_filetree(&source_dir, 42, false).unwrap();

    let out = run(&[
        "--driver", drv,
        "-r",
        "--verify-checksum",
        source_dir.to_str().unwrap(),
        dest_dir.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    compare_trees(&source_dir, &dest_dir.join("random_tree")).unwrap();
}

#[test]
fn checksum_without_flag_no_verification() {
    let dir = tempdir_rel().unwrap();
    let source = dir.path().join("source.bin");
    let dest = dir.path().join("dest.bin");

    let data = rand_data(1024);
    File::create(&source).unwrap().write_all(&data).unwrap();

    let out = run(&[
        source.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(files_match(&source, &dest));

    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(!stderr.contains("Checksum"));
    assert!(!stderr.contains("verification"));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn checksum_various_sizes(drv: &str) {
    let sizes = [
        1,
        10,
        100,
        1024,
        4096,
        64 * 1024,
        128 * 1024,
        256 * 1024,
        512 * 1024,
        1024 * 1024,
    ];

    for size in sizes {
        let dir = tempdir_rel().unwrap();
        let source = dir.path().join(format!("size_{}.bin", size));
        let dest = dir.path().join(format!("size_{}_copy.bin", size));

        let data = rand_data(size);
        File::create(&source).unwrap().write_all(&data).unwrap();

        let out = run(&[
            "--driver", drv,
            "--verify-checksum",
            source.to_str().unwrap(),
            dest.to_str().unwrap(),
        ]).unwrap();

        assert!(out.status.success(), "Failed for size: {}", size);
        assert!(files_match(&source, &dest));
    }
}
