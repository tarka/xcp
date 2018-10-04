use core::result;
use failure::{Error, Fail};

#[derive(Debug, Fail)]
pub enum XcpError {
    #[fail(display = "Unknown error. Placeholder, should never happen.")]
    Unknown,
}

pub type Result<T> = result::Result<T, Error>;
