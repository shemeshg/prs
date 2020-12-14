#[cfg(feature = "clipboard")]
pub mod clip_revert;

use clap::ArgMatches;

use super::Matcher;

/// The internal matcher.
pub struct InternalMatcher<'a> {
    root: &'a ArgMatches<'a>,
    _matches: &'a ArgMatches<'a>,
}

impl<'a: 'b, 'b> InternalMatcher<'a> {
    /// Get the internal clipboard revert sub command, if matched.
    #[cfg(feature = "clipboard")]
    pub fn clip_revert(&'a self) -> Option<clip_revert::ClipRevertMatcher> {
        clip_revert::ClipRevertMatcher::with(&self.root)
    }
}

impl<'a> Matcher<'a> for InternalMatcher<'a> {
    fn with(root: &'a ArgMatches) -> Option<Self> {
        root.subcommand_matches("_internal")
            .map(|matches| InternalMatcher {
                root,
                _matches: matches,
            })
    }
}
