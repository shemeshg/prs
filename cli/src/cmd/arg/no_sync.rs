use clap::Arg;

use super::{CmdArg, CmdArgFlag};

/// The no-sync argument.
pub struct ArgNoSync {}

impl CmdArg for ArgNoSync {
    fn name() -> &'static str {
        "no-sync"
    }

    fn build() -> Arg {
        Arg::new("no-sync")
            .long("no-sync")
            .short('D')
            .alias("keep-dirty")
            .alias("sync-no-sync")
            .alias("sync-keep-dirty")
            .num_args(0)
            .global(true)
            // This prevents: sync before action, committing changes, sync after action
            .help("Do not commit and sync changes, keep store dirty")
    }
}

impl CmdArgFlag for ArgNoSync {}
