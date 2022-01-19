use proc_macro2::{Literal, TokenStream};
use quote::quote_spanned;

use super::inputs::prelude::*;

pub struct HelpParts {
    pub items: Option<TokenStream>,
}

fn emit_bool(span: Span, b: bool) -> TokenStream {
    if b {
        quote_spanned! { span => true }
    } else {
        quote_spanned! { span => false }
    }
}

struct ArgUsage<'a> {
    name: &'a str,
    required: bool,
    rest: bool,
}

fn emit_usage(docs: &CommandDocs) -> TokenStream {
    let CommandDocs { span, usage, .. } = docs;
    let ids = usage.ids.iter().map(|i| Literal::string(i));
    let args = usage
        .required
        .iter()
        .map(|n| ArgUsage {
            name: n,
            required: true,
            rest: false,
        })
        .chain(usage.optional.iter().map(|n| ArgUsage {
            name: n,
            required: false,
            rest: false,
        }))
        .chain(match usage.rest {
            RestArg::None => None,
            RestArg::Optional(ref n) => Some(ArgUsage {
                name: n,
                required: false,
                rest: true,
            }),
            RestArg::Required(ref n) => Some(ArgUsage {
                name: n,
                required: true,
                rest: true,
            }),
        })
        .map(
            |ArgUsage {
                 name,
                 required,
                 rest,
             }| {
                let name = Literal::string(name);
                let required = emit_bool(*span, required);
                let rest = emit_bool(*span, rest);

                quote_spanned! { *span =>
                    ::docbot::ArgumentUsage {
                        name: #name,
                        is_required: #required,
                        is_rest: #rest,
                    }
                }
            },
        );
    let desc = Literal::string(&usage.desc);

    quote_spanned! { *span =>
        ::docbot::CommandUsage {
            ids: &[#(#ids),*],
            args: &[#(#args),*],
            desc: #desc
        }
    }
}

fn emit_desc(docs: &CommandDocs) -> TokenStream {
    let summary = docs.summary.as_ref().map_or_else(
        || quote_spanned! { docs.span => None },
        |summary| {
            let summary = Literal::string(summary);

            quote_spanned! { docs.span => Some(#summary) }
        },
    );

    let args = docs.args.iter().map(|(name, required, desc)| {
        let name = Literal::string(name);
        let required = emit_bool(docs.span, *required);
        let desc = Literal::string(desc);

        quote_spanned! { docs.span =>
            ::docbot::ArgumentDesc {
                name: #name,
                is_required: #required,
                desc: #desc,
            }
        }
    });

    let examples = docs.examples.as_ref().map_or_else(
        || quote_spanned! { docs.span => None },
        |examples| {
            let examples = Literal::string(examples);

            quote_spanned! { docs.span => Some(#examples) }
        },
    );

    quote_spanned! { docs.span =>
        ::docbot::CommandDesc {
            summary: #summary,
            args: &[#(#args),*],
            examples: #examples,
        }
    }
}

pub fn emit(input: &InputData) -> HelpParts {
    let topic_arms;
    let general_help;

    match input.commands {
        Commands::Struct {
            command: Command { ref docs, .. },
            ..
        } => {
            let usage = emit_usage(docs);
            let desc = emit_desc(docs);

            general_help = quote_spanned! { docs.span =>
                ::docbot::HelpTopic::Command(#usage, #desc)
            };

            topic_arms = vec![quote_spanned! { docs.span => Some(Self::Id) => &__GENERAL }];
        },
        Commands::Enum {
            ref docs,
            ref variants,
            ..
        } => {
            let summary = docs.summary.as_ref().map_or_else(
                || quote_spanned! { docs.span => None },
                |summary| {
                    let summary = Literal::string(summary);

                    quote_spanned! { docs.span => Some(#summary) }
                },
            );

            let commands = variants.iter().map(
                |CommandVariant {
                     command: Command { docs, .. },
                     ..
                 }| emit_usage(docs),
            );

            general_help = quote_spanned! { docs.span =>
                ::docbot::HelpTopic::CommandSet(#summary, &[#(#commands),*])
            };

            topic_arms = variants
                .iter()
                .map(
                    |CommandVariant {
                         span,
                         ident,
                         command: Command { docs, .. },
                         ..
                     }| {
                        let usage = emit_usage(docs);
                        let desc = emit_desc(docs);

                        quote_spanned! { *span =>
                            Some(Self::Id::#ident) => {
                                static __TOPIC: ::docbot::HelpTopic = ::docbot::HelpTopic::Command(
                                    #usage,
                                    #desc,
                                );

                                &__TOPIC
                            }
                        }
                    },
                )
                .collect();
        },
    }

    // Quote variables
    let name = input.ty;
    let (impl_vars, ty_vars, where_clause) = input.generics.split_for_impl();

    let items = if true {
        Some(quote_spanned! { input.span =>
            impl #impl_vars ::docbot::Help for #name #ty_vars #where_clause {
                fn help(__topic: Option<Self::Id>) -> &'static ::docbot::HelpTopic {
                    static __GENERAL: ::docbot::HelpTopic = #general_help;

                    match __topic {
                        #(#topic_arms,)*
                        None => &__GENERAL,
                    }
                }
            }
        })
    } else {
        None
    };

    HelpParts { items }
}
