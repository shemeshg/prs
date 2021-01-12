use std::io::Write;

use anyhow::Result;
use clap::ArgMatches;
use prs_lib::{store::Store, types::Plaintext};
use thiserror::Error;

use crate::cmd::matcher::{show::ShowMatcher, Matcher};
use crate::util::skim;

/// Show secret action.
pub struct Show<'a> {
    cmd_matches: &'a ArgMatches<'a>,
}

impl<'a> Show<'a> {
    /// Construct a new show action.
    pub fn new(cmd_matches: &'a ArgMatches<'a>) -> Self {
        Self { cmd_matches }
    }

    /// Invoke the show action.
    pub fn invoke(&self) -> Result<()> {
        // Create the command matchers
        let matcher_show = ShowMatcher::with(self.cmd_matches).unwrap();

        let store = Store::open(matcher_show.store()).map_err(Err::Store)?;
        let secret = skim::select_secret(&store, matcher_show.query()).ok_or(Err::NoneSelected)?;

        let mut plaintext = prs_lib::crypto::decrypt_file(&secret.path).map_err(Err::Read)?;

        // Trim plaintext to first line or property
        if matcher_show.first_line() {
            plaintext = plaintext.first_line()?;
        } else if let Some(property) = matcher_show.property() {
            plaintext = plaintext.property(property).map_err(Err::Property)?;
        }

        print(plaintext)
    }
}

/// Print the given plaintext to stdout.
// TODO: move to shared module
pub(crate) fn print(plaintext: Plaintext) -> Result<()> {
    std::io::stdout()
        .write_all(plaintext.unsecure_ref())
        .map_err(Err::Print)?;
    let _ = std::io::stdout().flush();
    Ok(())
}

#[derive(Debug, Error)]
pub enum Err {
    #[error("failed to access password store")]
    Store(#[source] anyhow::Error),

    #[error("no secret selected")]
    NoneSelected,

    #[error("failed to read secret")]
    Read(#[source] anyhow::Error),

    #[error("failed to print secret to stdout")]
    Print(#[source] std::io::Error),

    #[error("failed to select property from secret")]
    Property(#[source] anyhow::Error),
}
