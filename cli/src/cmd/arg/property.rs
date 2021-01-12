use clap::{Arg, ArgMatches};

use super::{CmdArg, CmdArgOption};

/// The property argument.
pub struct ArgProperty {}

impl CmdArg for ArgProperty {
    fn name() -> &'static str {
        "property"
    }

    fn build<'b, 'c>() -> Arg<'b, 'c> {
        Arg::with_name("property")
            .long("property")
            .short("p")
            .alias("prop")
            .value_name("NAME")
            .global(true)
            .help("Select a specific property")
    }
}

impl<'a> CmdArgOption<'a> for ArgProperty {
    type Value = Option<&'a str>;

    fn value<'b: 'a>(matches: &'a ArgMatches<'b>) -> Self::Value {
        Self::value_raw(matches)
    }
}
