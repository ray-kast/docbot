use syn::{spanned::Spanned, DeriveInput};

use crate::Result;

pub mod command;
pub mod field;

pub mod prelude {
    pub use proc_macro2::Span;
    pub use syn::{Generics, Ident, Variant, Visibility};

    pub use super::{
        command::{Command, CommandVariant, Commands},
        field::{FieldInfo, FieldInfos, FieldMode},
        InputData,
    };
    pub use crate::{
        docs::{CommandDocs, CommandSetDocs, CommandUsage, RestArg},
        opts::{CommandOpts, FieldOpts},
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

pub fn assemble(input: &DeriveInput) -> Result<InputData> {
    let commands = Commands::new(input)?;

    Ok(InputData {
        span: input.span(),
        vis: &input.vis,
        ty: &input.ident,
        generics: &input.generics,

        commands,
    })
}
