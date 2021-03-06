#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::cargo)]
#![deny(rustdoc::broken_intra_doc_links, missing_debug_implementations)]
#![allow(clippy::module_name_repetitions)]

//! Create a chatbot command interface using a docopt-like API

use std::{
    convert::Infallible,
    fmt,
    fmt::{Display, Formatter},
    str::FromStr,
};

use thiserror::Error;

#[cfg(feature = "strsim")]
mod did_you_mean;
mod fold_error;
mod fold_help;
mod tokenize;

#[cfg(feature = "strsim")]
pub use did_you_mean::did_you_mean;
pub use fold_error::{Downcast, FoldError, SimpleFoldError};
pub use fold_help::{FoldHelp, SimpleFoldHelp};
pub use tokenize::tokenize_str_simple;

/// Error type for failures when parsing a command ID
#[derive(Error, Debug)]
pub enum IdParseError {
    /// No IDs matched the given string
    #[error("no ID match for {0:?}")]
    NoMatch(String, &'static [&'static str]),
    /// Multiple IDs could match the given string
    ///
    /// Usually a result of specifying too few characters
    #[error("ambiguous ID {0:?}, could be any of {}", .0.join(", "))]
    Ambiguous(&'static [&'static str], String),
}

/// Error type for failures when parsing a command path
#[derive(Error, Debug)]
pub enum PathParseError {
    /// The iterator returned None immediately
    #[error("no values given for command path, expected one of {}", .0.join(", "))]
    Incomplete(&'static [&'static str]),
    /// A component command ID could not be parsed
    #[error("failed to parse command ID")]
    BadId(#[from] IdParseError),
    /// Extra values were provided
    #[error("trailing argument {0:?}")]
    Trailing(String),
}

/// Identifies an argument to a command
#[derive(Clone, Copy, Debug)]
pub struct ArgumentName {
    /// The ID of the command
    pub cmd: &'static str,
    /// The name of the argument
    pub arg: &'static str,
}

impl Display for ArgumentName {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_fmt(format_args!("{:?} of {:?}", self.arg, self.cmd))
    }
}

/// Error type for failures when parsing a command
#[derive(Error, Debug)]
pub enum CommandParseError {
    /// The iterator returned None immediately
    #[error("no values in command parse input")]
    NoInput,
    /// The command ID could not be parsed
    #[error("failed to parse command ID")]
    BadId(#[from] IdParseError),
    /// A required argument was missing
    #[error("missing required argument {0}")]
    MissingRequired(ArgumentName),
    /// `TryFrom::try_from` failed for an argument
    #[error("failed to convert argument {0} from a string")]
    BadConvert(ArgumentName, anyhow::Error),
    /// Extra arguments were provided
    #[error("trailing argument {1:?} of {0:?}")]
    Trailing(&'static str, String),
    /// A subcommand failed to parse
    #[error("failed to parse subcommand {0:?}")]
    Subcommand(&'static str, Box<CommandParseError>),
}

impl From<Infallible> for CommandParseError {
    fn from(i: Infallible) -> CommandParseError { match i {} }
}

#[doc(no_inline)]
pub use anyhow::Error as Anyhow;
#[doc(inline)]
pub use docbot_derive::*;

/// A parsable, identifiable command or family of commands
pub trait Command: Sized {
    /// The type of the command ID
    type Id: CommandId;

    /// The type of a valid command path starting at this command
    type Path: CommandPath<Id = Self::Id>;

    /// Try to parse a sequence of arguments as a command
    ///
    /// # Errors
    /// Should return an error for syntax or command-not-found errors, or for
    /// any errors while parsing arguments.
    fn parse<I: IntoIterator<Item = S>, S: AsRef<str>>(iter: I) -> Result<Self, CommandParseError>;

    /// Return an ID uniquely describing the base type of this command.
    fn id(&self) -> Self::Id;
}

/// A command ID, convertible to and from a string
pub trait CommandId: Copy + FromStr<Err = IdParseError> + Display {
    /// List all possible valid names that can be parsed, including aliases
    fn names() -> &'static [&'static str];

    /// Get the canonical name for an ID
    fn to_str(&self) -> &'static str;
}

/// A chain of command IDs representing a command or subcommand
pub trait CommandPath: From<Self::Id> {
    /// The ID type of the first path element
    type Id: CommandId;

    /// Try to parse a sequence of arguments as a command path
    ///
    /// # Errors
    /// Should return an error if any individual ID cannot be parsed correctly
    /// or if extra values are provided.
    fn parse<I: IntoIterator<Item = S>, S: AsRef<str>>(iter: I) -> Result<Self, PathParseError>;

    /// Try to parse a sequence of arguments as a command path, returning `None`
    /// if no input was given
    ///
    /// # Errors
    /// This function should return the same errors as [`parse`](Self::parse)
    /// but never [`NoInput`](PathParseError::NoInput)
    fn parse_opt<I: IntoIterator<Item = S>, S: AsRef<str>>(
        iter: I,
    ) -> Result<Option<Self>, PathParseError> {
        Self::parse(iter).map_or_else(
            |e| match e {
                PathParseError::Incomplete(_) => Ok(None),
                e => Err(e),
            },
            |p| Ok(Some(p)),
        )
    }

    /// Get the first element in this path
    fn head(&self) -> Self::Id;
}

impl<T: CommandId> CommandPath for T {
    type Id = Self;

    fn parse<I: IntoIterator<Item = S>, S: AsRef<str>>(iter: I) -> Result<Self, PathParseError> {
        let mut iter = iter.into_iter();
        let head = iter
            .next()
            .ok_or_else(|| PathParseError::Incomplete(Self::Id::names()))?;

        if let Some(s) = iter.next() {
            return Err(PathParseError::Trailing(s.as_ref().into()));
        }

        head.as_ref().parse().map_err(PathParseError::BadId)
    }

    fn head(&self) -> Self::Id { *self }
}

/// Usage description for an argument
#[derive(Debug, Clone)]
pub struct ArgumentUsage {
    /// The name of the argument
    pub name: &'static str,
    /// Whether the argument is required
    pub is_required: bool,
    /// Whether the argument is a rest parameter
    pub is_rest: bool,
}

/// Usage description for a command
#[derive(Debug, Clone)]
pub struct CommandUsage {
    /// The possible IDs of this command
    pub ids: &'static [&'static str],
    /// Usage descriptions for this command's arguments
    pub args: &'static [ArgumentUsage],
    /// A short description
    pub desc: &'static str,
}

/// Detailed description of a command argument
#[derive(Debug, Clone)]
pub struct ArgumentDesc {
    /// The name of the argument
    pub name: &'static str,
    /// Whether the argument is required
    pub is_required: bool,
    /// A detailed description of the argument
    pub desc: &'static str,
}

/// Detailed description of a command
#[derive(Debug, Clone)]
pub struct CommandDesc {
    /// A detailed summary of the command's behavior
    pub summary: Option<&'static str>,
    /// Descriptions of the command's arguments
    pub args: &'static [ArgumentDesc],
    /// Example uses of the command
    pub examples: Option<&'static str>,
}

/// A generic help topic
#[derive(Debug, Clone)]
pub enum HelpTopic {
    /// A help topic referring to a single command
    Command(CommandUsage, CommandDesc),
    /// A help topic referring to a set of commands, prefaced by an optional
    /// summary
    CommandSet(Option<&'static str>, &'static [CommandUsage]),
    /// A custom help topic
    Custom(&'static str),
}

/// A command with associated help topics
pub trait Help: Command {
    /// Retrieve the help topic corresponding to the given ID.
    fn help<U: Into<Self::Path>>(topic: Option<U>) -> &'static HelpTopic;
}

/// Common traits and types used with this crate
pub mod prelude {
    pub use super::{Command, CommandId, Docbot, FoldError, FoldHelp, Help};
}
