use std::borrow::Cow;

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref COMMAND_ARG_RE: Regex =
        Regex::new(r#"\s*(?:([^'"\s]\S*)|'([^']*)'|"((?:[^"\\]|\\.)*)")"#).unwrap();
    static ref COMMAND_DQUOTE_ESCAPE_RE: Regex = Regex::new(r"\\(.)").unwrap();
}

/// Performs simple tokenization of a string with minimal support for single-
/// and double-quoting
pub fn tokenize_str_simple(s: &str) -> impl Iterator<Item = Cow<str>> {
    COMMAND_ARG_RE.captures_iter(s).map(|cap| {
        cap.get(3).map_or_else(
            || {
                cap.get(2)
                    .unwrap_or_else(|| cap.get(1).unwrap_or_else(|| unreachable!()))
                    .as_str()
                    .into()
            },
            |dquote| COMMAND_DQUOTE_ESCAPE_RE.replace_all(dquote.as_str(), "$1"),
        )
    })
}
