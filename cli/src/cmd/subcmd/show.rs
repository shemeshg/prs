use clap::{App, Arg, SubCommand};

/// The show command definition.
pub struct CmdShow;

impl CmdShow {
    pub fn build<'a, 'b>() -> App<'a, 'b> {
        SubCommand::with_name("show")
            .about("Display a secret")
            .alias("s")
            .alias("cat")
            .alias("display")
            .arg(
                Arg::with_name("first")
                    .long("first")
                    .short("f")
                    .help("Show only the first line of the secret"),
            )
    }
}
