use std::{
    io::BufRead,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::ArgMatches;
use compress_io::compress::CompressIo;

use super::Config;

impl Config {
    pub fn from_matches(m: &ArgMatches) -> anyhow::Result<Self> {
        let input_files: Vec<_> = {
            let x = m
                .get_many::<PathBuf>("input")
                .expect("Missing required inputs");

            let mut v = x.map(|y| y.to_owned()).collect::<Vec<_>>();
            if (v.len() & 1) != 0 {
                return Err(anyhow!(
                    "Number of input files is not even (expecting pairs"
                ));
            }
            v.sort_unstable();
            v
        };

        let threads = m
            .get_one::<u64>("threads")
            .map(|x| *x as usize)
            .unwrap_or_else(num_cpus::get);

        let num_files = input_files.len() >> 1;

        let ignore_multibase_deletions = m.get_flag("ignore_multibase_deletions");
        let ignore_multiple_deletions = m.get_flag("ignore_multiple_deletions");
        let ignore_multiple_mutations = m.get_flag("ignore_multiple_mutations");
        let ignore_multiple_modifications = m.get_flag("ignore_multiple_modifications");
        let view_file = m.get_flag("view");

        let readers = m
            .get_one::<u64>("readers")
            .map(|x| *x as usize)
            .unwrap_or_else(|| {
                // No point having more readers than files or physical cores
                let i = num_cpus::get_physical().min(num_files);
                // One reader should easily be able to supply 4 process threads
                let j = (threads >> 2).max(1);
                i.min(j)
            });

        let min_qual = m
            .get_one::<u8>("min_qual")
            .copied()
            .expect("Missing default for min_qual");
        let max_overlap_divergence = m
            .get_one::<u32>("max_overlap_divergence")
            .copied()
            .expect("Missing default for min_qual");
        let max_length_divergence = m
            .get_one::<u32>("max_length_divergence")
            .copied()
            .expect("Missing default for min_qual");
        let output_prefix = m
            .get_one::<String>("output_prefix")
            .map(|s| s.to_owned())
            .expect("Missing default for output_prefix");

        let reference = read_reference(
            m.get_one::<PathBuf>("reference")
                .expect("Missing reference"),
        )?;
        Ok(Self {
            min_qual,
            output_prefix,
            threads,
            readers,
            reference,
            input_files,
            max_length_divergence,
            max_overlap_divergence,
            ignore_multibase_deletions,
            ignore_multiple_mutations,
            ignore_multiple_deletions,
            ignore_multiple_modifications,
            view_file,
        })
    }
}

fn read_reference(p: &Path) -> anyhow::Result<Vec<u8>> {
    debug!("Opening reference file");

    let mut rdr = CompressIo::new()
        .path(p)
        .bufreader()
        .with_context(|| format!("Could not open reference file {}", p.display()))?;

    info!("Reading from reference file {}", p.display());

    let mut s = String::new();
    let mut rf = Vec::new();
    let mut first = true;

    loop {
        let l = rdr
            .read_line(&mut s)
            .with_context(|| "Error reading from reference file")?;
        if l == 0 {
            break;
        }
        if s.starts_with('>') {
            if first { first = false } else { break }
        } else {
            rf.extend_from_slice(s.trim_end().as_bytes());
        }
        s.clear();
    }

    info!("Read reference ({} bases)", rf.len());
    Ok(rf)
}
