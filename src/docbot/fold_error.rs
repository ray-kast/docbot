use std::{fmt, fmt::Write};

use super::{Anyhow, ArgumentName, CommandParseError, IdParseError};

/// Helper for downcasting [`anyhow::Error`] into possible `docbot` errors
#[derive(Debug)]
pub enum Downcast {
    /// The error contained a [`CommandParseError`]
    CommandParse(CommandParseError),
    /// The error contained an [`IdParseError`]
    IdParse(IdParseError),
    /// The error was unable to be downcast
    Other(Anyhow),
}

impl From<Anyhow> for Downcast {
    fn from(anyhow: Anyhow) -> Self {
        anyhow.downcast().map_or_else(
            |e| e.downcast().map_or_else(Self::Other, Self::IdParse),
            Self::CommandParse,
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

    /// Handle a [`CommandParseError`]
    fn fold_command_parse(&self, err: CommandParseError) -> Self::Output {
        match err {
            CommandParseError::NoInput => self.no_input(),
            CommandParseError::BadId(i) => self.bad_id(i),
            CommandParseError::MissingRequired(ArgumentName { cmd, arg }) => {
                self.missing_required(cmd, arg)
            },
            CommandParseError::BadConvert(ArgumentName { cmd, arg }, e) => {
                self.bad_convert(cmd, arg, self.fold_anyhow(e))
            },
            CommandParseError::Trailing(c, t) => self.trailing(c, t),
            CommandParseError::Subcommand(s, e) => {
                self.subcommand(s, self.fold_command_parse(Box::into_inner(e)))
            },
        }
    }

    /// Handle a value of [`IdParseError::NoMatch`]
    fn no_id_match(&self, given: String, available: &'static [&'static str]) -> Self::Output;

    /// Handle a value of [`IdParseError::Ambiguous`]
    fn ambiguous_id(&self, possible: &'static [&'static str], given: String) -> Self::Output;

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

impl FoldError for SimpleFoldError {
    type Output = Result<String, fmt::Error>;

    fn no_id_match(&self, given: String, available: &'static [&'static str]) -> Self::Output {
        let mut s = String::new();

        write!(
            s,
            "Not sure what you mean by {:?}.  Available options are: ",
            given
        )?;

        Self::write_options(&mut s, available)?;

        Ok(s)
    }

    fn ambiguous_id(&self, possible: &'static [&'static str], given: String) -> Self::Output {
        let mut s = String::new();

        write!(s, "Not sure what you mean by {:?}.  Could be: ", given)?;

        Self::write_options(&mut s, possible)?;

        Ok(s)
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
        Ok(format!("Subcommand '{}' failed: {:?}", subcmd, inner))
    }

    fn other(&self, error: anyhow::Error) -> Self::Output { Ok(format!("{:?}", error)) }
}
