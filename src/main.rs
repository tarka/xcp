
mod errors;

use crate::errors::Result;

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "xcp",
            about = "Copy SOURCE to DEST, or multiple SOURCE(s) to DIRECTORY.")]
struct Opts {
    /// Explain what is being done
    #[structopt(short = "v", long = "verbose")]
    debug: bool,

    #[structopt(parse(from_os_str))]
    source: PathBuf,

    #[structopt(parse(from_os_str))]
    dest: PathBuf,
}



fn main() -> Result<()> {
    let opt = Opts::from_args();

    println!("Hello, world!");

    Ok(())
}
