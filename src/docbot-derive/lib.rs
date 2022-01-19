#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::cargo)]
#![deny(rustdoc::broken_intra_doc_links, missing_debug_implementations)]
#![allow(clippy::module_name_repetitions)]
#![feature(proc_macro_diagnostic)]

//! Derive macro for the docbot crate

mod attrs;
mod bits;
mod docs;
mod inputs;
mod opts;
mod trie;

use bits::{help::HelpParts, id::IdParts, parse::ParseParts, path::PathParts};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote_spanned;
use syn::{parse_macro_input, spanned::Spanned, DeriveInput};

pub(crate) type Error = (anyhow::Error, Span);
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

/// Construct a new command or set of commands from doc comments
#[proc_macro_derive(Docbot, attributes(docbot))]
pub fn derive_docbot(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);

    let inputs = match inputs::assemble(&input) {
        Ok(s) => s,
        Err((e, s)) => {
            s.unwrap()
                .error(format!("Macro execution failed:\n{:?}", e))
                .emit();
            return TokenStream::new();
        },
    };

    let id_parts = bits::id::emit(&inputs);
    let path_parts = bits::path::emit(&inputs, &id_parts);
    let parse_parts = bits::parse::emit(&inputs, &id_parts, &path_parts);
    let help_parts = bits::help::emit(&inputs);

    // Quote variables
    let IdParts {
        items: id_items, ..
    } = id_parts;
    let PathParts {
        items: path_items, ..
    } = path_parts;
    let ParseParts { items: parse_items } = parse_parts;
    let HelpParts { items: help_items } = help_parts;

    let toks = quote_spanned! { input.span() =>
        #id_items
        #path_items
        #parse_items
        #help_items
    };

    // eprintln!("{}", toks);

    toks.into()
}
