use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote_spanned};

use super::{id::IdParts, inputs::prelude::*};

pub struct PathParts {
    pub is_id: bool,
    pub ty: Ident,
    pub items: TokenStream,
}

pub fn emit(input: &InputData, id_parts: &IdParts) -> PathParts {
    let ty;
    let def;

    // let is_id = match input.commands {
    //     Commands::Struct(Command { ref docs, .. }) => docs.usage.,
    //     Commands::Enum(..) => todo!(),
    // };
    let is_id = true;

    if is_id {
        ty = id_parts.ty.clone();
        def = None;
    } else {
        let vis = input.vis;
        let id_ty = &id_parts.ty;
        let doc = Literal::string(&format!("Command path for commands of type {}", input.ty));

        let variants = vec![quote_spanned! { input.span => todo!() }];

        ty = format_ident!("{}Path", input.ty, span = input.ty.span());
        def = Some(quote_spanned! { input.span =>
            #[doc = #doc]
            #vis enum #ty {
                #(#variants),*
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
