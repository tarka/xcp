
use crate::errors::{Error};
use std::io::{Error as IOError, ErrorKind as IOKind};

pub fn to_err(kind: IOKind, desc: &str) -> Error {
    IOError::new(kind, desc).into()
}
