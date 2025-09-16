mod align;
mod counts;
mod read;

use super::cli::Config;

pub fn process(cfg: &Config) -> anyhow::Result<()> {
    let mut stats = counts::Stats::new(cfg.reference().len());

    for p in cfg.input_files().chunks_exact(2) {
        read::read_files(cfg, p, &mut stats)?
    }

    stats.output(cfg)
}
