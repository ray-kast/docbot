use proc_macro2::TokenStream;
use quote::quote_spanned;

use super::{id::IdParts, path::PathParts};
use crate::inputs::prelude::*;

pub struct ParseParts {
    pub items: TokenStream,
}

fn collect_rest(
    span: Span,
    cmd_opts: &CommandOpts,
    field_mode: &FieldMode,
    field_opts: &FieldOpts,
    name: &str,
    iter: &Ident,
    id: &Ident,
) -> TokenStream {
    if cmd_opts.subcommand {
        quote_spanned! { span =>
            ::docbot::Command::parse(#iter).map_err(|e| ::docbot::CommandParseError::Subcommand(
                ::docbot::CommandId::to_str(&#id),
                ::std::boxed::Box::new(e),
            ))
        }
    } else if field_opts.path {
        let parse = if field_mode.required() {
            quote_spanned! { span => parse }
        } else {
            quote_spanned! { span => parse_opt }
        };

        quote_spanned! { span =>
            ::docbot::CommandPath::#parse(#iter).map_err(|e| {
                ::docbot::CommandParseError::BadConvert(
                    ::docbot::ArgumentName {
                        cmd: ::docbot::CommandId::to_str(&#id),
                        arg: #name
                    },
                    ::docbot::Anyhow::from(e)
                )
            })
        }
    } else {
        quote_spanned! { span =>
            #iter
                .map(|s| {
                    s.as_ref().parse().map_err(|e| {
                        ::docbot::CommandParseError::BadConvert(
                            ::docbot::ArgumentName {
                                cmd: ::docbot::CommandId::to_str(&#id),
                                arg: #name,
                            },
                            ::docbot::Anyhow::from(e)
                        )
                    })
                })
                .collect::<::std::result::Result<_, _>>()
        }
    }
}

fn ctor_fields(
    span: Span,
    Command {
        opts: cmd_opts,
        docs,
        fields,
        ..
    }: &Command,
    path: TokenStream,
    iter: &Ident,
    id: &Ident,
) -> TokenStream {
    let process_arg = |FieldInfo {
                           opts, name, mode, ..
                       }: &FieldInfo| match mode {
        FieldMode::Required => quote_spanned! { span =>
            #iter
                .next()
                .ok_or_else(|| ::docbot::CommandParseError::MissingRequired(
                    ::docbot::ArgumentName {
                        cmd: ::docbot::CommandId::to_str(&#id),
                        arg: #name
                    }
                ))?
                .as_ref()
                .parse()
                .map_err(|e| ::docbot::CommandParseError::BadConvert(
                    ::docbot::ArgumentName {
                        cmd: ::docbot::CommandId::to_str(&#id),
                        arg: #name,
                    },
                    ::docbot::Anyhow::from(e),
                ))?
        },
        FieldMode::Optional => quote_spanned! { span =>
            #iter
                .next()
                .map(|s| s.as_ref().parse())
                .transpose()
                .map_err(|e| ::docbot::CommandParseError::BadConvert(
                    ::docbot::ArgumentName {
                        cmd: ::docbot::CommandId::to_str(&#id),
                        arg: #name,
                    },
                    ::docbot::Anyhow::from(e),
                ))?
        },
        FieldMode::RestRequired => {
            let peekable = Ident::new("__peek", span);
            let collected = collect_rest(span, cmd_opts, mode, opts, name, &peekable, id);

            quote_spanned! { span =>
                {
                    let mut #peekable = #iter.peekable();

                    if let Some(..) = #peekable.peek() {
                        #collected
                    } else {
                        Err(::docbot::CommandParseError::MissingRequired(
                            ::docbot::ArgumentName {
                                cmd: ::docbot::CommandId::to_str(&#id),
                                arg: #name,
                            }
                        ))
                    }
                }?
            }
        },
        FieldMode::RestOptional => {
            let collected = collect_rest(span, cmd_opts, mode, opts, name, iter, id);

            quote_spanned! { span => #collected? }
        },
    };

    let ret = match fields {
        FieldInfos::Unit => path,
        FieldInfos::Unnamed(u) => {
            let args = u.iter().map(process_arg);

            quote_spanned! { span => #path (#(#args),*) }
        },
        FieldInfos::Named(n) => {
            let args = n.iter().map(|(id, arg)| {
                let arg = process_arg(arg);

                quote_spanned! { span => #id: #arg }
            });

            quote_spanned! { span => #path { #(#args),* } }
        },
    };

    if let RestArg::None = docs.usage.rest {
        let check = quote_spanned! { span =>
            if let Some(__trail) = #iter.next() {
                return Err(::docbot::CommandParseError::Trailing(
                        ::docbot::CommandId::to_str(&#id),
                        __trail.as_ref().into(),
                        ));
            }
        };

        quote_spanned! { span =>
            {
                let __rest = #ret;
                #check;
                __rest
            }
        }
    } else {
        ret
    }
}

pub fn emit(input: &InputData, id_parts: &IdParts, path_parts: &PathParts) -> ParseParts {
    let iter = Ident::new("__iter", input.span);
    let id = Ident::new("__id", input.span);
    let id_ty = &id_parts.ty;

    let ctors: Vec<_> = match input.commands {
        Commands::Struct { ref command, .. } => {
            let ctor = ctor_fields(
                input.span,
                command,
                quote_spanned! { input.span => Self },
                &iter,
                &id,
            );

            vec![quote_spanned! { input.span => #id_ty => #ctor }]
        },
        Commands::Enum { ref variants, .. } => variants
            .iter()
            .map(
                |CommandVariant {
                     span,
                     ident,
                     command,
                     ..
                 }| {
                    let ctor = ctor_fields(
                        *span,
                        command,
                        quote_spanned! { *span => Self::#ident },
                        &iter,
                        &id,
                    );

                    quote_spanned! { *span => #id_ty::#ident => #ctor }
                },
            )
            .collect(),
    };

    // Quote variables
    let name = input.ty;
    let (impl_vars, ty_vars, where_clause) = input.generics.split_for_impl();
    let path_ty = &path_parts.ty;
    let id_get_fn = &id_parts.get_fn;

    let items = quote_spanned! { input.span =>
        impl #impl_vars ::docbot::Command for #name #ty_vars #where_clause {
            type Id = #id_ty;
            type Path = #path_ty;

            fn parse<
                I: IntoIterator<Item = S>,
                S: AsRef<str>,
            >(#iter: I) -> ::std::result::Result<Self, ::docbot::CommandParseError> {
                let mut #iter = #iter.into_iter().fuse();

                let #id: #id_ty = #iter
                   .next()
                   .ok_or(::docbot::CommandParseError::NoInput)?
                   .as_ref()
                   .parse()?;

                Ok(match #id {
                    #(#ctors),*
                })
            }

            fn id(&self) -> Self::Id { #id_get_fn }
        }
    };

    ParseParts { items }
}
