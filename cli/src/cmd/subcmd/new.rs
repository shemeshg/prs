use clap::{App, Arg, SubCommand};

/// The new command definition.
pub struct CmdNew;

impl CmdNew {
    pub fn build<'a, 'b>() -> App<'a, 'b> {
        SubCommand::with_name("new")
            .about("New secret")
            .alias("n")
            .arg(
                Arg::with_name("DEST")
                    .help("Secret destination path")
                    .required(true),
            )
    }
}
