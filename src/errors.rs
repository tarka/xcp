use core::result;
use failure::{Fail};

#[derive(Debug, Fail)]
pub enum XcpError {
    #[fail(display = "Unknown error. Placeholder, should never happen.")]
    Unknown,
}

pub use failure::Error;
pub type Result<T> = result::Result<T, Error>;
