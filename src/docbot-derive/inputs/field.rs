use std::collections::HashMap;

use anyhow::anyhow;
use syn::{spanned::Spanned, Fields, Type};

use super::prelude::*;
use crate::{attrs, Result};

pub enum FieldMode {
    Required,
    Optional,
    RestRequired,
    RestOptional,
}

impl FieldMode {
    pub fn required(&self) -> bool { matches!(self, Self::Required | Self::RestRequired) }

    pub fn rest(&self) -> bool { matches!(self, Self::RestRequired | Self::RestOptional) }
}

#[allow(clippy::manual_non_exhaustive)]
pub struct FieldInfo<'a> {
    pub span: Span,
    pub opts: FieldOpts,
    pub name: String,
    pub ty: &'a Type,
    pub mode: FieldMode,
    _priv: (),
}

pub enum FieldInfos<'a> {
    Unit,
    Unnamed(Vec<FieldInfo<'a>>),
    Named(Vec<(Ident, FieldInfo<'a>)>),
}

impl<'a> FieldInfos<'a> {
    pub fn new(span: Span, usage: &CommandUsage, fields: &'a Fields) -> Result<Self> {
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
                        let field = map
                            .remove(&i)
                            .ok_or_else(|| (anyhow!("could not locate field {}", i), span))?;
                        let span = field.span();

                        Ok(FieldInfo {
                            span,
                            opts: attrs::parse_field(&field.attrs, span)?,
                            mode,
                            name: name.into(),
                            ty: &field.ty,
                            _priv: (),
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
                    let field = map
                        .remove(name)
                        .ok_or_else(|| (anyhow!("could not locate field {:?}", name), span))?;
                    let span = field.span();

                    Ok((
                        syn::parse_str(name).map_err(|e| (e.into(), span))?,
                        FieldInfo {
                            span,
                            opts: attrs::parse_field(&field.attrs, span)?,
                            mode,
                            name: name.into(),
                            ty: &field.ty,
                            _priv: (),
                        },
                    ))
                })
                .collect::<Result<_>>()
                .map(FieldInfos::Named)
            },
        }?;

        if args.iter().len() != fields.iter().len() {
            return Err((anyhow!("mismatched number of fields and arguments"), span));
        }

        Ok(args)
    }

    pub fn iter(&self) -> Iter {
        match self {
            Self::Unit => Iter::Unit,
            Self::Unnamed(u) => Iter::Unnamed(u.iter()),
            Self::Named(n) => Iter::Named(n.iter()),
        }
    }
}

pub enum Iter<'a> {
    Unit,
    Unnamed(std::slice::Iter<'a, FieldInfo<'a>>),
    Named(std::slice::Iter<'a, (Ident, FieldInfo<'a>)>),
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a FieldInfo<'a>;

    #[inline]
    fn next(&mut self) -> Option<&'a FieldInfo<'a>> {
        match self {
            Self::Unit => None,
            Self::Unnamed(u) => u.next(),
            Self::Named(n) => n.next().map(|(_, f)| f),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = match self {
            Self::Unit => 0,
            Self::Unnamed(u) => u.len(),
            Self::Named(n) => n.len(),
        };

        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {}
