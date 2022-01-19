use std::{fmt, fmt::Write};

use super::{Anyhow, ArgumentName, CommandParseError, IdParseError, PathParseError};

/// Helper for downcasting [`anyhow::Error`] into possible `docbot` errors
#[derive(Debug)]
pub enum Downcast {
    /// The error contained a [`CommandParseError`]
    CommandParse(CommandParseError),
    /// The error contained an [`IdParseError`]
    IdParse(IdParseError),
    /// The error contained a [`PathParseError`]
    PathParse(PathParseError),
    /// The error was unable to be downcast
    Other(Anyhow),
}

macro_rules! try_downcast {
    ($id:ident => $($var:expr),+ $(,)?) => { try_downcast!(@map $id => $($var),+) };

    (@map $id:ident => $then:expr, $($else:expr),+) => {
        $id.downcast().map_or_else(try_downcast!(@cls $($else),+), $then)
    };

    (@cls $then:expr, $($else:expr),+) => { |e| try_downcast!(@map e => $then, $($else),+) };
    (@cls $then:expr) => { $then };
}

impl From<Anyhow> for Downcast {
    fn from(anyhow: Anyhow) -> Self {
        try_downcast!(anyhow =>
            Self::CommandParse,
            Self::IdParse,
            Self::PathParse,
            Self::Other,
        )
    }
}

/// Helper trait for processing and formatting errors from `docbot`
pub trait FoldError {
    /// The output type of a processed error
    type Output;

    /// Handle an error which may or may not contain a `docbot` error
    fn fold_anyhow(&self, err: Anyhow) -> Self::Output {
        match err.into() {
            Downcast::CommandParse(c) => self.fold_command_parse(c),
            Downcast::IdParse(i) => self.fold_id_parse(i),
            Downcast::PathParse(p) => self.fold_path_parse(p),
            Downcast::Other(o) => self.other(o),
        }
    }

    /// Handle an [`IdParseError`]
    fn fold_id_parse(&self, err: IdParseError) -> Self::Output {
        match err {
            IdParseError::NoMatch(given, available) => self.no_id_match(given, available),
            IdParseError::Ambiguous(possible, given) => self.ambiguous_id(possible, given),
        }
    }

    /// Handle a [`PathParseError`]
    fn fold_path_parse(&self, err: PathParseError) -> Self::Output {
        match err {
            PathParseError::Incomplete(available) => self.incomplete_path(available),
            PathParseError::BadId(err) => self.bad_path_id(err),
            PathParseError::Trailing(extra) => self.trailing_path(extra),
        }
    }

    /// Handle a [`CommandParseError`]
    fn fold_command_parse(&self, err: CommandParseError) -> Self::Output {
        match err {
            CommandParseError::NoInput => self.no_input(),
            CommandParseError::BadId(err) => self.bad_id(err),
            CommandParseError::MissingRequired(ArgumentName { cmd, arg }) => {
                self.missing_required(cmd, arg)
            },
            CommandParseError::BadConvert(ArgumentName { cmd, arg }, err) => {
                self.bad_convert(cmd, arg, self.fold_anyhow(err))
            },
            CommandParseError::Trailing(cmd, extra) => self.trailing(cmd, extra),
            CommandParseError::Subcommand(subcmd, err) => {
                self.subcommand(subcmd, self.fold_command_parse(*err))
            },
        }
    }

    /// Handle a value of [`IdParseError::NoMatch`]
    fn no_id_match(&self, given: String, available: &'static [&'static str]) -> Self::Output;

    /// Handle a value of [`IdParseError::Ambiguous`]
    fn ambiguous_id(&self, possible: &'static [&'static str], given: String) -> Self::Output;

    /// Handle a value of [`PathParseError::NoInput`]
    fn incomplete_path(&self, possible: &'static [&'static str]) -> Self::Output;

    /// Handle a value of [`PathParseError::BadId`]
    fn bad_path_id(&self, err: IdParseError) -> Self::Output { self.fold_id_parse(err) }

    /// Handle a value of [`PathParseError::Trailing`]
    fn trailing_path(&self, extra: String) -> Self::Output;

    /// Handle a value of [`CommandParseError::NoInput`]
    fn no_input(&self) -> Self::Output;

    /// Handle a value of [`CommandParseError::BadId`]
    fn bad_id(&self, err: IdParseError) -> Self::Output { self.fold_id_parse(err) }

    /// Handle a value of [`CommandParseError::MissingRequired`]
    fn missing_required(&self, cmd: &'static str, arg: &'static str) -> Self::Output;

    /// Handle a value of [`CommandParseError::BadConvert`]
    fn bad_convert(
        &self,
        cmd: &'static str,
        arg: &'static str,
        inner: Self::Output,
    ) -> Self::Output;

    /// Handle a value of [`CommandParseError::Trailing`]
    fn trailing(&self, cmd: &'static str, extra: String) -> Self::Output;

    /// Handle a value of [`CommandParseError::Subcommand`]
    fn subcommand(&self, subcmd: &'static str, inner: Self::Output) -> Self::Output;

    /// Handle an error that couldn't be downcast to a `docbot` error
    fn other(&self, error: Anyhow) -> Self::Output;
}

/// A basic implementation of [`FoldError`] that outputs a string describing the
/// error.
#[derive(Debug, Clone, Copy)]
pub struct SimpleFoldError;

impl SimpleFoldError {
    /// Format a list of possible command options
    ///
    /// # Errors
    /// This function fails if `w` throws an error when writing.
    pub fn write_options<S: fmt::Display>(
        mut w: impl Write,
        opts: impl IntoIterator<Item = S>,
    ) -> fmt::Result {
        opts.into_iter().enumerate().try_for_each(|(i, opt)| {
            if i != 0 {
                write!(w, ", ")?;
            }

            write!(w, "'{}'", opt)
        })
    }
}

#[cfg(feature = "strsim")]
use crate::did_you_mean;

/// Stub out did_you_mean when not available
#[cfg(not(feature = "strsim"))]
fn did_you_mean(_: impl std::any::Any, _: impl std::any::Any) -> std::iter::Empty<String> {
    std::iter::empty()
}

impl FoldError for SimpleFoldError {
    type Output = Result<String, fmt::Error>;

    fn no_id_match(&self, given: String, available: &'static [&'static str]) -> Self::Output {
        let mut s = String::new();

        write!(s, "Not sure what you mean by {:?}.", given)?;

        let mut dym = did_you_mean(given, available).peekable();

        if dym.peek().is_some() {
            s.push_str("  Did you mean: ");

            Self::write_options(&mut s, dym)?;
        } else if !available.is_empty() {
            s.push_str("  Available options are: ");

            Self::write_options(&mut s, available)?;
        }

        Ok(s)
    }

    fn ambiguous_id(&self, possible: &'static [&'static str], given: String) -> Self::Output {
        let mut s = String::new();

        write!(s, "Not sure what you mean by {:?}.  Could be: ", given)?;

        Self::write_options(&mut s, possible)?;

        Ok(s)
    }

    fn incomplete_path(&self, possible: &'static [&'static str]) -> Self::Output {
        let mut s = String::new();

        write!(s, "Incomplete command path, expected one of: ")?;

        Self::write_options(&mut s, possible)?;

        Ok(s)
    }

    fn trailing_path(&self, extra: String) -> Self::Output {
        Ok(format!("Unexpected extra path argument {:?}", extra))
    }

    fn no_input(&self) -> Self::Output { Ok(String::new()) }

    fn missing_required(&self, cmd: &'static str, arg: &'static str) -> Self::Output {
        Ok(format!(
            "Missing required argument '{}' to command '{}'",
            arg, cmd
        ))
    }

    fn bad_convert(
        &self,
        cmd: &'static str,
        arg: &'static str,
        inner: Self::Output,
    ) -> Self::Output {
        Ok(format!(
            "Couldn't parse argument '{}' of command '{}': {}",
            arg, cmd, inner?
        ))
    }

    fn trailing(&self, cmd: &'static str, extra: String) -> Self::Output {
        Ok(format!(
            "Unexpected extra argument {:?} to '{}'",
            extra, cmd
        ))
    }

    fn subcommand(&self, subcmd: &'static str, inner: Self::Output) -> Self::Output {
        Ok(format!("Subcommand '{}' failed: {}", subcmd, inner?))
    }

    fn other(&self, error: anyhow::Error) -> Self::Output { Ok(format!("{:?}", error)) }
}
