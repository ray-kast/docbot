use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote_spanned};

use super::id::IdParts;
use crate::inputs::prelude::*;

pub struct PathParts {
    pub is_id: bool,
    pub ty: Ident,
    pub items: TokenStream,
}

fn get_pats(
    span: Span,
    iter: &Ident,
    command: &Command,
) -> (
    Option<TokenStream>,
    Option<TokenStream>,
    Option<TokenStream>,
    Option<TokenStream>,
) {
    if command.opts.subcommand {
        let field = command.fields.iter().next().unwrap();
        let inner = field.ty;

        (
            Some(quote_spanned! { span => (Option<Box<<#inner as ::docbot::Command>::Path>>) }),
            Some(
                quote_spanned! { span => (::docbot::CommandPath::parse_opt(#iter)?.map(Box::new)) },
            ),
            Some(quote_spanned! { span => (..) }),
            Some(quote_spanned! { span => (None) }),
        )
    } else {
        (None, None, None, None)
    }
}

fn handle_variant(
    input_ty: &Ident,
    id_ty: &Ident,
    iter: &Ident,
    CommandVariant {
        ident,
        command,
        span,
        ..
    }: &CommandVariant,
) -> (TokenStream, (TokenStream, (TokenStream, TokenStream))) {
    let doc = Literal::string(&format!("Path for {}::{}", input_ty, ident));

    let (var_pat, parse_pat, head_pat, from_id_pat) = get_pats(*span, iter, command);

    (
        quote_spanned! { *span => #[doc = #doc] #ident #var_pat },
        (
            quote_spanned! { *span => #id_ty::#ident => Self::#ident #parse_pat },
            (
                quote_spanned! { *span => Self::#ident #head_pat => #id_ty::#ident },
                quote_spanned! { *span => #id_ty::#ident => Self::#ident #from_id_pat },
            ),
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
        let from_id;

        match input.commands {
            Commands::Struct { ref command, .. } => {
                let (body_pat, parse_pat, head_pat, from_id_pat) =
                    get_pats(input.span, &iter, command);

                def_body = quote_spanned! { input.span => struct #ty #body_pat; };
                parse = quote_spanned! { input.span => #id_ty => Self #parse_pat };
                head = quote_spanned! { input.span => Self #head_pat => #id_ty };
                from_id = quote_spanned! { input.span => #id_ty => Self #from_id_pat };
            },
            Commands::Enum { ref variants, .. } => {
                let (path_vars, (parse_vars, (head_vars, from_id_vars))): (
                    Vec<_>,
                    (Vec<_>, (Vec<_>, Vec<_>)),
                ) = variants
                    .iter()
                    .map(|v| handle_variant(input.ty, id_ty, &iter, v))
                    .unzip();

                def_body = quote_spanned! { input.span => enum #ty { #(#path_vars),* } };
                parse = quote_spanned! { input.span => #(#parse_vars),* };
                head = quote_spanned! { input.span => #(#head_vars),* };
                from_id = quote_spanned! { input.span => #(#from_id_vars),* };
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
                       .ok_or_else(|| {
                           ::docbot::PathParseError::Incomplete(#id_ty::names())
                       })?
                       .as_ref()
                       .parse()? {
                        #parse
                    })
                }

                fn head(&self) -> #id_ty { match self { #head } }
            }

            impl ::std::convert::From<#id_ty> for #ty {
                fn from(id: #id_ty) -> Self { match id { #from_id } }
            }
        });
    }

    let items = quote_spanned! { input.span =>
        #def
    };

    PathParts { is_id, ty, items }
}
