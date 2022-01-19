use std::collections::HashMap;

use anyhow::anyhow;
use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::{spanned::Spanned, Data, DeriveInput, Fields};

use crate::{attrs, trie::Trie, Result};

pub mod prelude {
    pub use proc_macro2::Span;
    pub use syn::{Generics, Ident, Variant, Visibility};

    pub use super::{
        Command, CommandVariant, Commands, FieldInfo, FieldInfos, FieldMode, InputData,
    };
    pub use crate::{
        docs::{CommandDocs, CommandSetDocs, CommandUsage, RestArg},
        opts::FieldOpts,
    };
}

use prelude::*;

pub struct InputData<'a> {
    pub span: Span,
    pub vis: &'a Visibility,
    pub ty: &'a Ident,
    pub generics: &'a Generics,

    pub commands: Commands<'a>,
}

pub enum Commands<'a> {
    Struct {
        id_trie: Trie<()>,
        command: Command,
    },
    Enum {
        docs: CommandSetDocs,
        id_trie: Trie<&'a Ident>,
        variants: Vec<CommandVariant<'a>>,
    },
}

pub struct Command {
    pub docs: CommandDocs,
    pub fields: FieldInfos,
}

pub struct CommandVariant<'a> {
    pub span: Span,
    pub ident: &'a Ident,
    pub pat: TokenStream,
    pub command: Command,
}

pub enum FieldMode {
    Required,
    Optional,
    RestRequired,
    RestOptional,
}

pub struct FieldInfo {
    pub opts: FieldOpts,
    pub name: String,
    pub mode: FieldMode,
}

pub enum FieldInfos {
    Unit,
    Unnamed(Vec<FieldInfo>),
    Named(Vec<(Ident, FieldInfo)>),
}

impl FieldInfos {
    fn new(span: Span, usage: &CommandUsage, fields: &Fields) -> Result<Self> {
        let mut args = usage
            .required
            .iter()
            .map(|n| (FieldMode::Required, n))
            .chain(usage.optional.iter().map(|n| (FieldMode::Optional, n)))
            .chain(match usage.rest {
                RestArg::None => None,
                RestArg::Optional(ref n) => Some((FieldMode::RestOptional, n)),
                RestArg::Required(ref n) => Some((FieldMode::RestRequired, n)),
            });

        let args = match fields {
            Fields::Unit if args.next().is_none() => Ok(FieldInfos::Unit),
            Fields::Unit => Err((anyhow!("could not locate any fields in unit type"), span)),
            Fields::Unnamed(u) => {
                let mut map: HashMap<_, _> = u.unnamed.iter().enumerate().collect();

                args.enumerate()
                    .map(|(i, (mode, name))| {
                        Ok(FieldInfo {
                            opts: attrs::parse_field(
                                &map.remove(&i)
                                    .ok_or_else(|| (anyhow!("could not locate field {}", i), span))?
                                    .attrs,
                                span,
                            )?,
                            mode,
                            name: name.into(),
                        })
                    })
                    .collect::<Result<_>>()
                    .map(FieldInfos::Unnamed)
            },
            Fields::Named(n) => {
                let mut map: HashMap<_, _> = n
                    .named
                    .iter()
                    .map(|f| (f.ident.as_ref().unwrap().to_string(), f))
                    .collect();

                args.map(|(mode, name)| {
                    Ok((
                        syn::parse_str(name).map_err(|e| (e.into(), span))?,
                        FieldInfo {
                            opts: attrs::parse_field(
                                &map.remove(name)
                                    .ok_or_else(|| {
                                        (anyhow!("could not locate field {:?}", name), span)
                                    })?
                                    .attrs,
                                span,
                            )?,
                            mode,
                            name: name.into(),
                        },
                    ))
                })
                .collect::<Result<_>>()
                .map(FieldInfos::Named)
            },
        }?;

        if args.len() != fields.iter().len() {
            return Err((anyhow!("mismatched number of fields and arguments"), span));
        }

        Ok(args)
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Unit => 0,
            Self::Unnamed(u) => u.len(),
            Self::Named(n) => n.len(),
        }
    }
}

pub fn assemble(input: &DeriveInput) -> Result<InputData> {
    let commands = match input.data {
        Data::Struct(ref s) => {
            let docs: CommandDocs = attrs::parse_outer(&input.attrs, input.span())?;
            let fields = FieldInfos::new(input.span(), &docs.usage, &s.fields)?;

            let id_trie = Trie::new(docs.usage.ids.iter().map(|i| (i.to_lowercase(), ())))
                .map_err(|e| (e.context("failed to construct command lexer"), input.span()))?;

            Commands::Struct {
                id_trie,
                command: Command { docs, fields },
            }
        },
        Data::Enum(ref e) => {
            let docs = attrs::parse_outer(&input.attrs, input.span())?;

            let variants = e
                .variants
                .iter()
                .map(|v| {
                    let docs = attrs::parse_variant(&v.attrs, v.span())?;
                    let fields = FieldInfos::new(v.span(), &docs.usage, &v.fields)?;

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
                        command: Command { docs, fields },
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
    };

    Ok(InputData {
        span: input.span(),
        vis: &input.vis,
        ty: &input.ident,
        generics: &input.generics,

        commands,
    })
}
