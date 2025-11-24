mod getters;
mod mk_config;

use std::path::PathBuf;

pub struct Config {
    min_qual: u8,
    output_prefix: String,
    threads: usize,
    readers: usize,
    reference: Vec<u8>,
    input_files: Vec<PathBuf>,
    ignore_multibase_deletions: bool,
    ignore_multiple_deletions: bool,
    ignore_multiple_mutations: bool,
    view_file: bool,
}
