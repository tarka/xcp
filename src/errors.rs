use core::result;
use failure::{Fail};

#[derive(Debug, Fail)]
pub enum XcpError {
    #[fail(display = "Failed to find filename.")]
    UnknownFilename,
}

pub use failure::Error;
pub type Result<T> = result::Result<T, Error>;
