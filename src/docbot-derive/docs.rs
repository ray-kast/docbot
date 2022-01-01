use std::collections::{hash_map::Entry, HashMap};

use anyhow::anyhow;
use lazy_static::lazy_static;
use proc_macro2::Span;
use regex::{Regex, RegexBuilder};

use crate::Result;

#[derive(Clone, Debug)]
pub enum RestArg {
    None,
    Optional(String),
    Required(String),
}

#[derive(Clone, Debug)]
pub struct CommandUsage {
    pub ids: Vec<String>,
    pub required: Vec<String>,
    pub optional: Vec<String>,
    pub rest: RestArg,
    pub desc: String,
}

#[derive(Debug)]
pub struct CommandDocs {
    pub span: Span,
    pub usage: CommandUsage,
    pub summary: Option<String>,
    pub args: Vec<(String, bool, String)>,
    pub examples: Option<String>,
}

pub struct CommandSetDocs {
    pub span: Span,
    pub summary: Option<String>,
}

pub trait ParseDocs: Sized {
    fn parse_docs(docs: Vec<(String, Span)>, fallback_span: Span) -> Result<Self>;

    fn no_docs() -> Result<Self, anyhow::Error>;
}

fn take_paragraph<I: Iterator<Item = (String, Span)>>(
    docs: &mut I,
    preserve_lines: bool,
) -> Option<(String, Span)> {
    let mut ret = String::new();
    let mut ret_span = None;

    for (string, span) in docs {
        let trimmed = string.trim();

        if trimmed.is_empty() {
            break;
        }

        if preserve_lines {
            ret.push_str(string.as_ref());
            ret.push('\n');
        } else {
            if !ret.is_empty() {
                ret.push(' ');
            }

            ret.push_str(trimmed);
        }

        ret_span = Some(ret_span.map_or(span, |r: Span| r.join(span).unwrap()));
    }

    ret_span.map(|s| (ret, s))
}

fn parse_usage_desc((par, span): (String, Span)) -> Result<CommandUsage> {
    lazy_static! {
        static ref USAGE_SPLIT_RE: Regex =
            Regex::new(r"^\s*`((?:[^`]|``)*)`(?:\s*:)?\s*(.*)\s*$").unwrap();
        static ref COMMAND_IDS_RE: Regex =
            Regex::new(r"^\s*(?:([^\(]\S*)|\(\s*([^\)]*)\))").unwrap();
        static ref PIPE_RE: Regex = Regex::new(r"\s*\|\s*").unwrap();
        static ref REQUIRED_ARG_RE: Regex =
            Regex::new(r"^\s*<([^>]{0,2}|[^>]*[^>\.]{3})>").unwrap();
        static ref OPTIONAL_ARG_RE: Regex =
            Regex::new(r"^\s*\[([^\]]{0,2}|[^\]]*[^\]\.]{3})\]").unwrap();
        static ref REST_ARG_RE: Regex =
            Regex::new(r"^\s*(?:<([^>]+)...>|\[([^\]]+)...\])").unwrap();
        static ref TRAILING_RE: Regex = Regex::new(r"\S").unwrap();
    }

    let (mut usage, desc) = {
        let caps = USAGE_SPLIT_RE.captures(&par).ok_or_else(|| {
            (
                anyhow!("Invalid usage paragraph, format should be `<usage>` <description>"),
                span,
            )
        })?;

        (caps.get(1).unwrap().as_str(), caps[2].to_owned())
    };

    let ids_match = COMMAND_IDS_RE.captures(usage).ok_or_else(|| {
        (
            anyhow!("invalid command ID specifier, expected e.g. 'foo' or '(foo|bar)'"),
            span,
        )
    })?;

    let ids = if let Some(cap) = ids_match.get(2) {
        PIPE_RE.split(cap.as_str()).map(Into::into).collect()
    } else {
        vec![ids_match[1].into()]
    };

    usage = &usage[ids_match.get(0).unwrap().end()..];

    let mut required = vec![];
    while let Some(req) = REQUIRED_ARG_RE.captures(usage) {
        required.push(req[1].into());

        usage = &usage[req.get(0).unwrap().end()..];
    }

    let mut optional = vec![];
    while let Some(opt) = OPTIONAL_ARG_RE.captures(usage) {
        optional.push(opt[1].into());

        usage = &usage[opt.get(0).unwrap().end()..];
    }

    let rest = REST_ARG_RE.captures(usage).map_or(RestArg::None, |rest| {
        usage = &usage[rest.get(0).unwrap().end()..];

        rest.get(2).map_or_else(
            || RestArg::Required(rest[1].into()),
            |cap| RestArg::Optional(cap.as_str().into()),
        )
    });

    if TRAILING_RE.is_match(usage) {
        return Err((anyhow!("trailing string {:?}", usage), span));
    }

    Ok(CommandUsage {
        ids,
        required,
        optional,
        rest,
        desc,
    })
}

fn relax_lines(s: impl AsRef<str>) -> String {
    lazy_static! {
        static ref LINE_RE: Regex = Regex::new(r"\s*\n\s*").unwrap();
    }

    LINE_RE.replace_all(s.as_ref().trim(), " ").into_owned()
}

fn parse_argument_lines(
    span: Span,
    usage: &CommandUsage,
    s: impl AsRef<str>,
) -> Result<Vec<(String, bool, String)>> {
    lazy_static! {
        static ref ARGUMENT_RE: Regex =
            RegexBuilder::new(r"^\s*([^\n\s](?:[^:\n]*[^:\n\s])?)\s*:\s*")
                .multi_line(true)
                .build()
                .unwrap();
    }

    let s = s.as_ref();
    let mut args = HashMap::new();
    let matches: Vec<_> = ARGUMENT_RE.captures_iter(s).collect();

    for (i, caps) in matches.iter().enumerate() {
        let end = matches
            .get(i + 1)
            .map_or_else(|| s.len(), |c| c.get(0).unwrap().start());

        match args.entry(caps[1].into()) {
            Entry::Occupied(o) => {
                return Err((
                    anyhow!("duplicate argument description {:?}", o.key()),
                    span,
                ));
            },
            Entry::Vacant(v) => {
                v.insert(relax_lines(&s[caps.get(0).unwrap().end()..end]));
            },
        }
    }

    if !s.is_empty() && args.is_empty() {
        return Err((anyhow!("unexpected argument description format"), span));
    }

    let expected_args: Vec<_> = usage
        .required
        .iter()
        .map(|a| (a, true))
        .chain(usage.optional.iter().map(|a| (a, false)))
        .chain(match usage.rest {
            RestArg::None => None,
            RestArg::Optional(ref a) => Some((a, false)),
            RestArg::Required(ref a) => Some((a, true)),
        })
        .collect();

    for (arg, _) in &expected_args {
        if args.get(*arg).is_none() {
            return Err((
                anyhow!(
                    "missing documentation for argument {:?} (have documentation for {:?})",
                    arg,
                    args.keys().collect::<Vec<_>>(),
                ),
                span,
            ));
        }
    }

    if args.len() != expected_args.len() {
        return Err((
            anyhow!(
                "mismatched number of argument descriptions (expected {}, got {})",
                expected_args.len(),
                args.len()
            ),
            span,
        ));
    }

    let args = expected_args
        .into_iter()
        .map(|(arg, req)| {
            let (key, val) = args.remove_entry(arg).unwrap();
            (key, req, val)
        })
        .collect();

    Ok(args)
}

impl ParseDocs for CommandDocs {
    fn parse_docs(docs: Vec<(String, Span)>, fallback_span: Span) -> Result<Self> {
        let span = docs
            .iter()
            .map(|(_, s)| s)
            .fold(None, |prev, curr| match prev {
                None => Some(Some(*curr)),
                Some(p) => Some(p.and_then(|p| p.join(*curr))),
            })
            .flatten()
            .unwrap_or(fallback_span);

        let mut docs = docs.into_iter();

        let usage_desc =
            take_paragraph(&mut docs, false).unwrap_or_else(|| (String::new(), fallback_span));
        let usage = parse_usage_desc(usage_desc)?;

        let mut summary = None;
        let mut args = None;
        let mut examples = None;

        while let Some((par, span)) = take_paragraph(&mut docs, true) {
            lazy_static! {
                static ref HEADER_RE: Regex = Regex::new(r"^\s*#\s*(\S+)\s*\n").unwrap();
            }

            let header_caps = HEADER_RE
                .captures(&par)
                .ok_or_else(|| (anyhow!("paragraph missing header"), span))?;
            let rest = &par[header_caps.get(0).unwrap().end()..];

            match header_caps[1].to_lowercase().as_ref() {
                "description" | "overview" | "summary" => {
                    if summary.is_some() {
                        return Err((anyhow!("multiple summary sections found"), span));
                    }

                    summary = Some(relax_lines(rest));
                },
                "arguments" | "parameters" => {
                    if args.is_some() {
                        return Err((anyhow!("multiple arguments sections found"), span));
                    }

                    args = Some(parse_argument_lines(span, &usage, rest)?);
                },
                "examples" => {
                    if examples.is_some() {
                        return Err((anyhow!("multiple examples sections found"), span));
                    }

                    examples = Some(relax_lines(rest));
                },
                _ => (),
            }
        }

        if usage.desc.trim().is_empty() {
            return Err((anyhow!("missing command description"), span));
        }

        let args = args.map_or_else(|| parse_argument_lines(span, &usage, ""), Ok)?;

        Ok(Self {
            span,
            usage,
            summary,
            args,
            examples,
        })
    }

    fn no_docs() -> Result<Self, anyhow::Error> { Err(anyhow!("missing doc comment for command")) }
}

impl ParseDocs for CommandSetDocs {
    fn parse_docs(docs: Vec<(String, Span)>, fallback_span: Span) -> Result<Self> {
        let span = docs
            .iter()
            .map(|(_, s)| s)
            .fold(None, |prev, curr| match prev {
                None => Some(Some(*curr)),
                Some(p) => Some(p.and_then(|p| p.join(*curr))),
            })
            .flatten()
            .unwrap_or(fallback_span);

        let mut docs = docs.into_iter();
        let mut summary = String::new();

        while let Some((par, span)) = take_paragraph(&mut docs, false) {
            if !summary.is_empty() {
                summary.push('\n');
            }

            summary.push_str(&par);

            let _ = span; // TODO: here just in case
        }

        let summary = summary.trim();
        let summary = if summary.is_empty() {
            None
        } else {
            Some(summary.into())
        };

        Ok(Self { span, summary })
    }

    fn no_docs() -> Result<Self, anyhow::Error> {
        Ok(Self {
            span: Span::call_site(),
            summary: None,
        })
    }
}

impl ParseDocs for () {
    fn parse_docs(_: Vec<(String, Span)>, _: Span) -> Result<Self> { Ok(()) }

    fn no_docs() -> Result<Self, anyhow::Error> { Ok(()) }
}
