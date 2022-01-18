use std::{fmt, fmt::Write};

use lazy_static::lazy_static;
use regex::Regex;

use super::{ArgumentDesc, ArgumentUsage, CommandDesc, CommandUsage, HelpTopic};

/// Helper trait for processing and formatting help topics from `docbot`
pub trait FoldHelp {
    /// The output type of the processed help topic
    type Output;

    /// Handle a raw [`ArgumentUsage`] struct
    #[inline]
    fn fold_argument_usage(&self, usage: &'static ArgumentUsage) -> Self::Output {
        self.argument_usage(usage.name, usage.is_required, usage.is_rest)
    }

    /// Handle a raw [`CommandUsage`] struct
    #[inline]
    fn fold_command_usage(&self, usage: &'static CommandUsage, long: bool) -> Self::Output {
        self.command_usage(
            usage.ids,
            usage.args.iter().map(|a| self.fold_argument_usage(a)),
            usage.desc,
            long,
        )
    }

    /// Handle a raw [`ArgumentDesc`] struct
    #[inline]
    fn fold_argument_desc(&self, desc: &'static ArgumentDesc) -> Self::Output {
        self.argument_desc(desc.name, desc.is_required, desc.desc)
    }

    /// Handle a raw [`CommandDesc`] struct
    #[inline]
    fn fold_command_desc(&self, desc: &'static CommandDesc) -> Self::Output {
        self.command_desc(
            desc.summary,
            desc.args.iter().map(|a| self.fold_argument_desc(a)),
            desc.examples,
        )
    }

    /// Handle a [`HelpTopic`]
    fn fold_topic(&self, topic: &'static HelpTopic) -> Self::Output {
        match topic {
            HelpTopic::Command(usage, desc) => self.command_topic(
                self.fold_command_usage(usage, true),
                self.fold_command_desc(desc),
            ),
            HelpTopic::CommandSet(summary, commands) => self.command_set_topic(
                *summary,
                commands.iter().map(|c| self.fold_command_usage(c, false)),
            ),
            HelpTopic::Custom(topic) => self.custom_topic(topic),
        }
    }

    /// Handle a value of [`HelpTopic::Command`]
    fn command_topic(&self, usage: Self::Output, desc: Self::Output) -> Self::Output;

    /// Handle a value of [`HelpTopic::CommandSet`]
    fn command_set_topic(
        &self,
        summary: Option<&'static str>,
        commands: impl IntoIterator<Item = Self::Output>,
    ) -> Self::Output;

    /// Handle a value of [`HelpTopic::Custom`]
    fn custom_topic(&self, topic: &'static str) -> Self::Output;

    /// Handle an argument within a command's usage line
    fn argument_usage(&self, name: &'static str, is_required: bool, is_rest: bool) -> Self::Output;

    /// Handle the usage line for a command
    ///
    /// `long` is a hint indicating whether the output value should be made
    /// terse.  When `false`, a more compact value should be produced.
    fn command_usage(
        &self,
        ids: &'static [&'static str],
        args: impl IntoIterator<Item = Self::Output>,
        desc: &'static str,
        long: bool,
    ) -> Self::Output;

    /// Handle an argument description line from a command description
    fn argument_desc(
        &self,
        name: &'static str,
        is_required: bool,
        desc: &'static str,
    ) -> Self::Output;

    /// Handle the description blocks for a command
    fn command_desc(
        &self,
        summary: Option<&'static str>,
        args: impl IntoIterator<Item = Self::Output>,
        examples: Option<&'static str>,
    ) -> Self::Output;
}

/// A basic implementation of [`FoldHelp`] outputting multiline strings akin to
/// POSIX command help text.
#[derive(Debug, Clone, Copy)]
pub struct SimpleFoldHelp;

impl SimpleFoldHelp {
    /// Format the ID(s) associated with a command
    ///
    /// # Errors
    /// This function fails if `w` throws an error when writing.
    pub fn write_command_ids<I: ExactSizeIterator<Item = S>, S: AsRef<str>>(
        mut w: impl Write,
        ids: impl IntoIterator<IntoIter = I>,
    ) -> fmt::Result {
        lazy_static! {
            static ref NON_WORD_RE: Regex = Regex::new(r"\s").unwrap();
        }

        let mut ids = ids.into_iter().peekable();
        let paren = ids.len() != 1 || {
            let id = ids.peek().unwrap_or_else(|| unreachable!()).as_ref();
            id.is_empty() || NON_WORD_RE.is_match(id)
        };

        if paren {
            write!(w, "(")?;
        }

        for (i, id) in ids.enumerate() {
            if i > 0 {
                write!(w, "|")?;
            }

            write!(w, "{}", id.as_ref())?;
        }

        if paren {
            write!(w, ")")?;
        }

        Ok(())
    }
}

impl FoldHelp for SimpleFoldHelp {
    type Output = Result<String, fmt::Error>;

    fn command_topic(&self, usage: Self::Output, desc: Self::Output) -> Self::Output {
        let usage = usage?;
        let desc = desc?;

        let mut s = String::new();

        s.push_str(&usage);

        if !(usage.is_empty() || desc.is_empty()) {
            s.push_str("\n\n");
        }

        s.push_str(&desc);

        Ok(s)
    }

    fn command_set_topic(
        &self,
        summary: Option<&'static str>,
        commands: impl IntoIterator<Item = Self::Output>,
    ) -> Self::Output {
        let mut s = String::new();

        if let Some(summary) = summary {
            s.push_str(summary);
        }

        let mut commands = commands.into_iter().peekable();

        if commands.peek().is_some() {
            if !s.is_empty() {
                s.push_str("\n\n");
            }

            s.push_str("COMMANDS");

            for cmd in commands {
                write!(s, "\n  {}", cmd?)?;
            }
        }

        Ok(s)
    }

    fn custom_topic(&self, topic: &'static str) -> Self::Output { Ok(topic.to_owned()) }

    fn argument_usage(&self, name: &'static str, is_required: bool, is_rest: bool) -> Self::Output {
        let mut s = String::new();

        s.push(if is_required { '<' } else { '[' });
        s.push_str(name);
        if is_rest {
            s.push_str("...");
        }
        s.push(if is_required { '>' } else { ']' });

        Ok(s)
    }

    fn command_usage(
        &self,
        ids: &'static [&'static str],
        args: impl IntoIterator<Item = Self::Output>,
        desc: &'static str,
        long: bool,
    ) -> Self::Output {
        let mut s = String::new();

        if long {
            s.push_str("USAGE: ");
        }

        Self::write_command_ids(&mut s, ids.iter().copied())?;

        for arg in args {
            if !s.is_empty() {
                s.push(' ');
            }

            s.push_str(&arg?);
        }

        if !s.is_empty() {
            if long {
                s.push('\n');
            } else {
                s.push_str(": ");
            }
        }
        s.push_str(desc);

        Ok(s)
    }

    fn argument_desc(
        &self,
        name: &'static str,
        is_required: bool,
        desc: &'static str,
    ) -> Self::Output {
        let mut s = String::new();

        s.push_str(name);

        if !is_required {
            s.push_str(" (optional)");
        }

        write!(s, ": {}", desc)?;

        Ok(s)
    }

    fn command_desc(
        &self,
        summary: Option<&'static str>,
        args: impl IntoIterator<Item = Self::Output>,
        examples: Option<&'static str>,
    ) -> Self::Output {
        let mut s = String::new();

        if let Some(summary) = summary {
            write!(s, "SUMMARY\n{}", summary)?;
        }

        let mut args = args.into_iter().peekable();

        if args.peek().is_some() {
            if !s.is_empty() {
                s.push_str("\n\n");
            }

            s.push_str("ARGUMENTS");

            for arg in args {
                write!(s, "\n  {}", arg?)?;
            }
        }

        if let Some(examples) = examples {
            if !s.is_empty() {
                s.push_str("\n\n");
            }

            write!(s, "EXAMPLES\n\n{}", examples)?;
        }

        Ok(s)
    }
}
