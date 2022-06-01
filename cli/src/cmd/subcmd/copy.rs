use clap::{Arg, Command};

use crate::cmd::arg::{ArgProperty, ArgQuery, ArgStore, ArgTimeout, CmdArg};

/// The copy command definition.
pub struct CmdCopy;

impl CmdCopy {
    pub fn build<'a>() -> Command<'a> {
        Command::new("copy")
            .alias("cp")
            .alias("c")
            .alias("yank")
            .alias("clip")
            .alias("clipboard")
            .about("Copy secret to clipboard")
            .arg(
                Arg::new("all")
                    .long("all")
                    .short('a')
                    .help("Copy whole secret, not just first line"),
            )
            .arg(ArgQuery::build())
            .arg(ArgTimeout::build())
            .arg(ArgStore::build())
            .arg(ArgProperty::build().conflicts_with("all"))
    }
}
