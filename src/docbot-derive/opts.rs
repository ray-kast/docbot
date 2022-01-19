use anyhow::anyhow;
use syn::{spanned::Spanned, Attribute, Meta, NestedMeta};

use crate::Result;

pub trait ParseOpts: Sized {
    fn parse_opts(attr: &Attribute) -> Result<Self>;

    fn no_opts() -> Result<Self, anyhow::Error>;
}

#[derive(Debug, Default)]
pub struct CommandOpts {
    pub subcommand: bool,
}

impl ParseOpts for CommandOpts {
    fn parse_opts(attr: &Attribute) -> Result<Self> {
        let meta = attr.parse_meta().map_err(|e| (e.into(), attr.span()))?;
        let mut ret = Self::default();

        match meta {
            Meta::List(l) => {
                for item in l.nested {
                    match item {
                        NestedMeta::Meta(Meta::Path(p)) if p.is_ident("subcommand") => {
                            if ret.subcommand {
                                return Err((anyhow!("duplicate subcommand specifier"), p.span()));
                            }

                            ret.subcommand = true;
                        },
                        i => {
                            return Err((
                                anyhow!("unexpected value in #[docbot] command attribute"),
                                i.span(),
                            ));
                        },
                    }
                }
            },
            _ => {
                return Err((
                    anyhow!("invalid #[docbot] attribute format, expected #[docbot(...)]",),
                    attr.span(),
                ));
            },
        }

        Ok(ret)
    }

    fn no_opts() -> Result<Self, anyhow::Error> { Ok(Self::default()) }
}

#[derive(Debug, Default)]
pub struct FieldOpts {
    pub path: bool,
}

impl ParseOpts for FieldOpts {
    fn parse_opts(attr: &Attribute) -> Result<Self> {
        let meta = attr.parse_meta().map_err(|e| (e.into(), attr.span()))?;
        let mut ret = Self::default();

        match meta {
            Meta::List(l) => {
                for item in l.nested {
                    match item {
                        NestedMeta::Meta(Meta::Path(p)) if p.is_ident("path") => {
                            if ret.path {
                                return Err((anyhow!("duplicate path specifier"), p.span()));
                            }

                            ret.path = true;
                        },
                        i => {
                            return Err((
                                anyhow!("unexpected value in #[docbot] field attribute"),
                                i.span(),
                            ));
                        },
                    }
                }
            },
            _ => {
                return Err((
                    anyhow!("invalid #[docbot] attribute format, expected #[docbot(...)]",),
                    attr.span(),
                ));
            },
        }

        Ok(ret)
    }

    fn no_opts() -> Result<Self, anyhow::Error> { Ok(FieldOpts::default()) }
}

impl ParseOpts for () {
    fn parse_opts(attr: &Attribute) -> Result<Self> {
        Err((anyhow!("unexpected #[docbot] attribute"), attr.span()))
    }

    fn no_opts() -> Result<Self, anyhow::Error> { Ok(()) }
}
