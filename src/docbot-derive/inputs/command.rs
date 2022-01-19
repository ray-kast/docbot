use anyhow::anyhow;
use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::{spanned::Spanned, Attribute, Data, DeriveInput, Fields};

use super::prelude::*;
use crate::{attrs, trie::Trie, Result};

#[allow(clippy::manual_non_exhaustive)]
pub struct Command<'a> {
    pub opts: CommandOpts,
    pub docs: CommandDocs,
    pub fields: FieldInfos<'a>,
    _priv: (),
}

impl<'a> Command<'a> {
    pub fn new(span: Span, attrs: &[Attribute], fields: &'a Fields) -> Result<Self> {
        let (opts, docs) = attrs::parse_command(attrs, span)?;
        let fields = FieldInfos::new(span, &docs.usage, fields)?;

        if opts.subcommand {
            let is_valid = {
                let mut it = fields.iter();

                it.len() == 1 && {
                    let field = it.next().unwrap();

                    !field.opts.path && field.mode.rest()
                }
            };

            if !is_valid {
                return Err((
                    anyhow!(
                        "Invalid structure for a subcommand, should be a single rest parameter"
                    ),
                    span,
                ));
            }
        }

        for field in fields.iter() {
            if field.opts.path && !field.mode.rest() {
                return Err((
                    anyhow!("Invalid path argument, should be a rest parameter"),
                    field.span,
                ));
            }
        }

        Ok(Self {
            opts,
            docs,
            fields,
            _priv: (),
        })
    }
}

pub struct CommandVariant<'a> {
    pub span: Span,
    pub ident: &'a Ident,
    pub pat: TokenStream,
    pub command: Command<'a>,
}

pub enum Commands<'a> {
    Struct {
        id_trie: Trie<()>,
        command: Command<'a>,
    },
    Enum {
        docs: CommandSetDocs,
        id_trie: Trie<&'a Ident>,
        variants: Vec<CommandVariant<'a>>,
    },
}

impl<'a> Commands<'a> {
    pub fn new(input: &'a DeriveInput) -> Result<Self> {
        Ok(match input.data {
            Data::Struct(ref s) => {
                let command = Command::new(input.span(), &input.attrs, &s.fields)?;

                let id_trie = Trie::new(
                    command
                        .docs
                        .usage
                        .ids
                        .iter()
                        .map(|i| (i.to_lowercase(), ())),
                )
                .map_err(|e| (e.context("failed to construct command lexer"), input.span()))?;

                Self::Struct { id_trie, command }
            },
            Data::Enum(ref e) => {
                let docs = attrs::parse_enum(&input.attrs, input.span())?;

                let variants = e
                    .variants
                    .iter()
                    .map(|v| {
                        Ok(CommandVariant {
                            ident: &v.ident,
                            pat: {
                                let id = &v.ident;
                                match v.fields {
                                    Fields::Named(..) => {
                                        quote_spanned! { v.span() => Self::#id { .. } }
                                    },
                                    Fields::Unnamed(..) => {
                                        quote_spanned! { v.span() => Self::#id(..) }
                                    },
                                    Fields::Unit => quote_spanned! { v.span() => Self::#id },
                                }
                            },
                            command: Command::new(v.span(), &v.attrs, &v.fields)?,
                            span: v.span(),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;

                let id_trie = Trie::new(variants.iter().flat_map(|v| {
                    v.command
                        .docs
                        .usage
                        .ids
                        .iter()
                        .map(move |i| (i.to_lowercase(), v.ident))
                }))
                .map_err(|e| (e.context("failed to construct command lexer"), input.span()))?;

                Commands::Enum {
                    docs,
                    id_trie,
                    variants,
                }
            },
            Data::Union(_) => {
                return Err((anyhow!("cannot derive Docbot on a union."), input.span()));
            },
        })
    }

    pub fn iter(&self) -> Iter {
        match self {
            Self::Struct { command, .. } => Iter::Struct(std::iter::once(command)),
            Self::Enum { variants, .. } => Iter::Enum(variants.iter()),
        }
    }
}

pub enum Iter<'a> {
    Struct(std::iter::Once<&'a Command<'a>>),
    Enum(std::slice::Iter<'a, CommandVariant<'a>>),
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Command<'a>;

    #[inline]
    fn next(&mut self) -> Option<&'a Command<'a>> {
        match self {
            Self::Struct(s) => s.next(),
            Self::Enum(e) => e.next().map(|v| &v.command),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = match self {
            Self::Struct(s) => s.len(),
            Self::Enum(e) => e.len(),
        };

        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {}
