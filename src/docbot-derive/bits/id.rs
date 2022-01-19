use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote_spanned, ToTokens};

use crate::inputs::prelude::*;

pub struct IdParts {
    pub ty: Ident,
    pub items: TokenStream,
    pub get_fn: TokenStream,
}

fn parse_no_match(span: Span, s: impl ToTokens) -> impl ToTokens {
    quote_spanned! { span =>
        Err(::docbot::IdParseError::NoMatch(#s.into(), <Self as ::docbot::CommandId>::names()))
    }
}

fn parse_ambiguous(span: Span, s: impl ToTokens, values: Vec<&str>) -> impl ToTokens {
    let expected = values.into_iter().map(Literal::string);

    quote_spanned! { span => Err(::docbot::IdParseError::Ambiguous(&[#(#expected),*], #s.into())) }
}

fn parse_resolve_ambiguous<'a, 'b, T: Eq + 'b>(values: Vec<&'a (String, T)>) -> Option<&'a T> {
    let mut iter = values.into_iter();

    let first = match iter.next() {
        Some((_, ref i)) => i,
        None => return None,
    };

    if iter.any(|(_, i)| i != first) {
        return None;
    }

    Some(first)
}

fn bits<'a>(
    input: &'a InputData,
) -> (
    Ident,
    Option<TokenStream>,
    Option<&'a Generics>,
    TokenStream,
) {
    let ty;
    let def;
    let generics;
    let get_fn;

    if input
        .commands
        .iter()
        .all(|c| matches!(c.fields, FieldInfos::Unit))
    {
        ty = input.ty.clone();
        def = None;
        generics = Some(input.generics);
        get_fn = quote_spanned! { input.span => *self }
    } else {
        let vis = input.vis;
        let doc = Literal::string(&format!("Identifier for commands of type {}", input.ty));

        ty = format_ident!("{}Id", input.ty, span = input.ty.span());

        let data;

        match input.commands {
            Commands::Struct { .. } => {
                data = quote_spanned! { input.span => struct #ty; };
                get_fn = quote_spanned! { input.span => #ty };
            },
            Commands::Enum { ref variants, .. } => {
                let id_vars = variants.iter().map(|CommandVariant { span, ident, .. }| {
                    let doc = Literal::string(&format!("Identifier for {}::{}", input.ty, ident));

                    quote_spanned! { *span => #[doc = #doc] #ident }
                });

                data = quote_spanned! { input.span => enum #ty { #(#id_vars),* } };

                let id_arms = variants.iter().map(
                    |CommandVariant {
                         span, pat, ident, ..
                     }| {
                        quote_spanned! { *span => #pat => #ty::#ident }
                    },
                );

                get_fn = quote_spanned! { input.span => match self { #(#id_arms),* } };
            },
        };

        def = Some(quote_spanned! { input.span =>
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            #[doc = #doc]
            #vis #data
        });
        generics = None;
    }

    (ty, def, generics, get_fn)
}

pub fn emit(input: &InputData) -> IdParts {
    let (ty, def, generics, get_fn) = bits(input);

    let parse_s = Ident::new("__str", input.span);
    let parse_iter = Ident::new("__iter", input.span);

    let lexer = match input.commands {
        Commands::Struct { ref id_trie, .. } => id_trie.root().to_lexer(
            input.span,
            &parse_iter,
            |()| quote_spanned! { input.span => Ok(#ty) },
            || parse_no_match(input.span, &parse_s),
            |v| parse_ambiguous(input.span, &parse_s, v),
            parse_resolve_ambiguous,
        ),
        Commands::Enum { ref id_trie, .. } => id_trie.root().to_lexer(
            input.span,
            &parse_iter,
            |i| quote_spanned! { input.span => Ok(#ty::#i) },
            || parse_no_match(input.span, &parse_s),
            |v| parse_ambiguous(input.span, &parse_s, v),
            parse_resolve_ambiguous,
        ),
    };

    let to_str_arms;
    let names;

    match input.commands {
        Commands::Struct {
            command: Command { ref docs, .. },
            ..
        } => {
            let value = Literal::string(&docs.usage.ids[0]);
            to_str_arms = vec![quote_spanned! { input.span => Self => #value }];

            names = docs.usage.ids.clone();
        },
        Commands::Enum { ref variants, .. } => {
            to_str_arms = variants
                .iter()
                .map(|CommandVariant { ident, command, .. }| {
                    let value = Literal::string(&command.docs.usage.ids[0]);
                    quote_spanned! { input.span => Self::#ident => #value }
                })
                .collect();

            names = variants
                .iter()
                .flat_map(|v| v.command.docs.usage.ids.iter())
                .cloned()
                .collect();
        },
    };

    // Quote variables
    let (impl_vars, ty_vars, where_clause) = generics.map_or((None, None, None), |generics| {
        let (imp, ty, whr) = generics.split_for_impl();
        (Some(imp), Some(ty), Some(whr))
    });

    let items = quote_spanned! { input.span =>
        #def

        impl #impl_vars ::std::str::FromStr for #ty #ty_vars #where_clause {
            type Err = ::docbot::IdParseError;

            fn from_str(#parse_s: &str) -> Result<Self, Self::Err> {
                let __to_lower = #parse_s.to_lowercase();
                let mut #parse_iter = __to_lower.chars();

                #lexer
            }
        }

        impl #impl_vars ::std::fmt::Display for #ty #ty_vars #where_clause {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(match self {
                    #(#to_str_arms),*
                })
            }
        }

        impl #impl_vars ::docbot::CommandId for #ty #ty_vars #where_clause {
            fn names() -> &'static [&'static str] { &[#(#names),*] }

            fn to_str(&self) -> &'static str {
                match self {
                    #(#to_str_arms),*
                }
            }
        }
    };

    IdParts { ty, items, get_fn }
}
