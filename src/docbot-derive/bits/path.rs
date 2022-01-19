use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote_spanned};

use super::id::IdParts;
use crate::inputs::prelude::*;

pub struct PathParts {
    pub is_id: bool,
    pub ty: Ident,
    pub items: TokenStream,
}

fn get_subcommand<'a>(command: &'a Command<'a>) -> Option<&'a FieldInfo<'a>> {
    if command.opts.subcommand {
        Some(command.fields.iter().next().unwrap())
    } else {
        None
    }
}

fn fields_pat(span: Span, field: &FieldInfo) -> TokenStream {
    let inner = field.ty;

    quote_spanned! { span => (Option<Box<<#inner as ::docbot::Command>::Path>>) }
}

fn parse_inner(span: Span, iter: &Ident) -> TokenStream {
    quote_spanned! { span =>
        (::docbot::CommandPath::parse_opt(#iter)?.map(Box::new))
    }
}

fn blank_pat(span: Span) -> TokenStream {
    quote_spanned! { span => (..) }
}

fn handle_variant(
    input_ty: &Ident,
    iter: &Ident,
    CommandVariant {
        ident,
        command,
        span,
        ..
    }: &CommandVariant,
) -> (TokenStream, (TokenStream, TokenStream)) {
    let doc = Literal::string(&format!("Path for {}::{}", input_ty, ident));

    let (var_pat, parse_pat, head_pat) = get_subcommand(command)
        .map(|field| {
            (
                fields_pat(*span, field),
                parse_inner(*span, iter),
                blank_pat(*span),
            )
        })
        .map_or((None, None, None), |(a, b, c)| (Some(a), Some(b), Some(c)));

    (
        quote_spanned! { *span => #[doc = #doc] #ident #var_pat },
        (
            quote_spanned! { *span => Self::Id::#ident => Self::#ident #parse_pat },
            quote_spanned! { *span => Self::#ident #head_pat => Self::Id::#ident },
        ),
    )
}

pub fn emit(input: &InputData, id_parts: &IdParts) -> PathParts {
    let ty;
    let def;

    let is_id = !input.commands.iter().any(|c| c.opts.subcommand);

    if is_id {
        ty = id_parts.ty.clone();
        def = None;
    } else {
        let vis = input.vis;
        let id_ty = &id_parts.ty;
        let iter = Ident::new("__iter", input.span);
        let doc = Literal::string(&format!("Command path for commands of type {}", input.ty));

        ty = format_ident!("{}Path", input.ty, span = input.ty.span());

        let def_body;
        let parse;
        let head;

        match input.commands {
            Commands::Struct { ref command, .. } => {
                let (body_pat, parse_pat, head_pat) = get_subcommand(command)
                    .map(|field| {
                        (
                            fields_pat(input.span, field),
                            parse_inner(input.span, &iter),
                            blank_pat(input.span),
                        )
                    })
                    .map_or((None, None, None), |(a, b, c)| (Some(a), Some(b), Some(c)));

                def_body = quote_spanned! { input.span => struct #ty #body_pat; };
                parse = quote_spanned! { input.span => Self::Id => Self #parse_pat };
                head = quote_spanned! { input.span => Self #head_pat => Self::Id };
            },
            Commands::Enum { ref variants, .. } => {
                let (path_vars, (parse_vars, head_vars)): (Vec<_>, (Vec<_>, Vec<_>)) = variants
                    .iter()
                    .map(|v| handle_variant(input.ty, &iter, v))
                    .unzip();

                def_body = quote_spanned! { input.span => enum #ty { #(#path_vars),* } };
                parse = quote_spanned! { input.span => #(#parse_vars),* };
                head = quote_spanned! { input.span => #(#head_vars),* };
            },
        }

        def = Some(quote_spanned! { input.span =>
            #[doc = #doc]
            #vis #def_body

            impl ::docbot::CommandPath for #ty {
                type Id = #id_ty;

                fn parse<I: IntoIterator<Item = S>, S: AsRef<str>>(
                    #iter: I
                ) -> ::std::result::Result<Self, ::docbot::PathParseError> {
                    let mut #iter = #iter.into_iter();

                    Ok(match #iter
                       .next()
                       .ok_or(::docbot::PathParseError::NoInput)?
                       .as_ref()
                       .parse()? {
                        #parse
                    })
                }

                fn head(&self) -> #id_ty {
                    match self {
                        #head
                    }
                }
            }

            impl ::std::convert::From<#id_ty> for #ty {
                fn from(id: #id_ty) -> Self { todo!() }
            }
        });
    }

    let items = quote_spanned! { input.span =>
        #def
    };

    PathParts { is_id, ty, items }
}
