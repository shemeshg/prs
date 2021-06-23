use regex::Regex;
use thiserror::Error;

/// Parse the given duration string from human readable format into seconds.
/// This method parses a string of time components to represent the given duration.
///
/// The following time units are used:
/// - `w`: weeks
/// - `d`: days
/// - `h`: hours
/// - `m`: minutes
/// - `s`: seconds
/// The following time strings can be parsed:
/// - `8w6d`
/// - `23h14m`
/// - `9m55s`
/// - `1s1s1s1s1s`
pub fn parse_duration(duration: &str) -> Result<usize, ParseDurationError> {
    // Build a regex to grab time parts
    let re = Regex::new(r"(?i)([0-9]+)(([a-z]|\s*$))")
        .expect("failed to compile duration parsing regex");

    // We must find any match
    if re.find(duration).is_none() {
        return Err(ParseDurationError::Empty);
    }

    // Parse each time part, sum it's seconds
    let mut seconds = 0;
    for capture in re.captures_iter(duration) {
        // Parse time value and modifier
        let number = capture[1]
            .parse::<usize>()
            .map_err(ParseDurationError::InvalidValue)?;
        let modifier = capture[2].trim().to_lowercase();

        // Multiply and sum seconds by modifier
        seconds += match modifier.as_str() {
            "" | "s" => number,
            "m" => number * 60,
            "h" => number * 60 * 60,
            "d" => number * 60 * 60 * 24,
            "w" => number * 60 * 60 * 24 * 7,
            m => return Err(ParseDurationError::UnknownIdentifier(m.into())),
        };
    }

    Ok(seconds)
}

/// Represents a duration parsing error.
#[derive(Debug, Error)]
pub enum ParseDurationError {
    /// The given duration string did not contain any duration part.
    #[error("given string did not contain any duration part")]
    Empty,

    /// A numeric value was invalid.
    #[error("duration part has invalid numeric value")]
    InvalidValue(std::num::ParseIntError),

    /// The given duration string contained an invalid duration modifier.
    #[error("duration part has unknown time identifier '{}'", _0)]
    UnknownIdentifier(String),
}
