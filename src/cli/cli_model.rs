use std::path::PathBuf;

use clap::{Arg, ArgAction, Command, command, value_parser};
use super::log_level::LogLevel;

pub(super) fn cli_model() -> Command {
    command!()
        .arg(
            Arg::new("timestamp")
                .short('X')
                .long("timestamp")
                .value_parser(value_parser!(stderrlog::Timestamp))
                .value_name("GRANULARITY")
                .default_value("none")
                .help("Prepend log entries with a timestamp"),
        )
        .arg(
            Arg::new("loglevel")
                .short('l')
                .long("loglevel")
                .value_name("LOGLEVEL")
                .value_parser(value_parser!(LogLevel))
                .ignore_case(true)
                .default_value("info")
                .help("Set log level"),
        )
        .arg(
            Arg::new("quiet")
                .action(ArgAction::SetTrue)
                .long("quiet")
                .conflicts_with("loglevel")
                .help("Silence all output"),
        )
        .arg(
            Arg::new("threads")
                .short('t')
                .long("threads")
                .value_parser(value_parser!(u64).range(1..))
                .value_name("INT")
                .help("Set number of process threads [default: available cores"),
        )
        .arg(
            Arg::new("readers")
                .short('r')
                .long("readers")
                .value_parser(value_parser!(u64).range(1..))
                .value_name("INT")
                .help("Set number of read threads [default: MIN (physical cores, no. input files)"),
        )
        .arg(
            Arg::new("min_qual")
                .long("min-qual")
                .short('q')
                .value_parser(value_parser!(u8))
                .value_name("QUAL")
                .default_value("0")
                .help("Minimum base quality to consider"),
        )
        .arg(
            Arg::new("ignore_multibase_deletions")
                .action(ArgAction::SetTrue)
                .long("ignore-multibase-deletions")
                .short('M')
                .help("Ignore read pairs with multibase deletions"),
        )
        .arg(
            Arg::new("ignore_multiple_deletions")
                .action(ArgAction::SetTrue)
                .long("ignore-multiple-deletions")
                .short('d')
                .help("Ignore read pairs with multiple deletions"),
        )
        .arg(
            Arg::new("ignore_multiple_mutations")
                .action(ArgAction::SetTrue)
                .long("ignore-multiple-mutations")
                .short('m')
                .help("Ignore read pairs with multiple mutations"),
        )
        .arg(
            Arg::new("ignore_multiple_modifications")
                .action(ArgAction::SetTrue)
                .long("ignore-multiple-modifications")
                .short('D')
                .help("Ignore read pairs with multiple modifications"),
        )
        .arg(
            Arg::new("view")
                .action(ArgAction::SetTrue)
                .long("view")
                .short('V')
                .help("Output view file"),
        )
        .arg(
            Arg::new("reference")
                .short('R')
                .long("reference")
                .value_parser(value_parser!(PathBuf))
                .value_name("FILE")
                .required(true)
                .help("Reference sequence FASTA"),
        )
        .arg(
            Arg::new("output_prefix")
                .long("output-prefix")
                .short('o')
                .value_parser(value_parser!(String))
                .value_name("OUTPUT PREFIX")
                .default_value("ampl_seq")
                .help("Prefix for output file"),
        )
        .arg(
            Arg::new("input")
                .value_parser(value_parser!(PathBuf))
                .value_name("INPUT")
                .action(ArgAction::Append)
                .num_args(2..)
                .required(true)
                .help("Input FASTQ file(s)"),
        )
}