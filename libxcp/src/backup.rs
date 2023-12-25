use std::path::Path;

use regex::Regex;

const NUM_SEP: char = '~';
const BAK_PATTTERN: &str = r"^~(\d+)~$";

pub fn is_num_backup(path: &Path) -> Option<u64> {
    let suf = path.extension()?
        .to_string_lossy();
    let cap = Regex::new(BAK_PATTTERN).unwrap()
        .captures(&suf)?;
    let num = cap.get(1)?
        .as_str()
        .parse::<u64>()
        .ok()?;
    Some(num)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_is_backup() {
        let f = PathBuf::from("/some/path/file.txt.~123~");
        let b = is_num_backup(&f);
        assert!(b.is_some());
        assert_eq!(123, b.unwrap());
    }

}
