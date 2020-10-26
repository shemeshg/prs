use clap::ArgMatches;

use anyhow::Result;
use prs_lib::{store::Store, types::Plaintext};
use thiserror::Error;

use crate::cmd::matcher::{generate::GenerateMatcher, MainMatcher, Matcher};
use crate::util;

/// Generate secret action.
pub struct Generate<'a> {
    cmd_matches: &'a ArgMatches<'a>,
}

impl<'a> Generate<'a> {
    /// Construct a new generate action.
    pub fn new(cmd_matches: &'a ArgMatches<'a>) -> Self {
        Self { cmd_matches }
    }

    /// Invoke the generate action.
    pub fn invoke(&self) -> Result<()> {
        // Create the command matchers
        let matcher_main = MainMatcher::with(self.cmd_matches).unwrap();
        let matcher_generate = GenerateMatcher::with(self.cmd_matches).unwrap();

        let store = Store::open(crate::STORE_DEFAULT_ROOT).map_err(Err::Store)?;

        let dest = matcher_generate.destination();

        // Normalize destination path
        let path = store
            .normalize_secret_path(dest, None, true)
            .map_err(Err::NormalizePath)?;

        // Generate secure passphrase plaintext
        let mut plaintext = Plaintext::from_string(chbs::passphrase());

        // Check if destination already exists, ask to merge if so
        if !matcher_main.force() && path.is_file() {
            eprintln!("A secret at '{}' already exists", path.display(),);
            if !util::prompt_yes("Merge?", Some(true), &matcher_main) {
                if !matcher_main.quiet() {
                    eprintln!("No secret generated");
                }
                util::quit();
            }

            // Append existing secret exept first line to new secret
            let existing = prs_lib::crypto::decrypt_file(&path)
                .and_then(|p| p.except_first_line())
                .map_err(Err::Read)?;
            if !existing.is_empty() {
                plaintext.append(existing, true);
            }
        }

        // Append from stdin
        if matcher_generate.stdin() {
            let extra = util::stdin_read_plaintext(!matcher_main.quiet());
            plaintext.append(extra, true);
        }

        // Edit in editor
        if matcher_generate.edit() {
            if let Some(changed) = util::edit(&plaintext).map_err(Err::Edit)? {
                plaintext = changed;
            }
        }

        // Confirm if empty secret should be stored
        if !matcher_main.force() && plaintext.is_empty() {
            if !util::prompt_yes(
                "Generated secret is empty. Save?",
                Some(true),
                &matcher_main,
            ) {
                util::quit();
            }
        }

        // Encrypt and write changed plaintext
        // TODO: select proper recipients (use from current file?)
        // TODO: log recipients to encrypt for
        let recipients = store.recipients()?;
        prs_lib::crypto::encrypt_file(&recipients, plaintext, &path).map_err(Err::Write)?;

        if !matcher_main.quiet() {
            eprintln!("Secret created");
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Err {
    #[error("failed to access password store")]
    Store(#[source] anyhow::Error),

    #[error("failed to normalize destination path")]
    NormalizePath(#[source] anyhow::Error),

    #[error("failed to edit secret in editor")]
    Edit(#[source] std::io::Error),

    #[error("failed to read existing secret")]
    Read(#[source] anyhow::Error),

    #[error("failed to write changed secret")]
    Write(#[source] anyhow::Error),
}
