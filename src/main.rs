#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

mod cli;
mod process;

fn main() -> anyhow::Result<()> {
    let cfg = cli::handle_cli()?;
    debug!("Options read in - starting processing");
    process::process(&cfg)
}
