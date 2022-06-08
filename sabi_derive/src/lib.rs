#![allow(unused)]
extern crate proc_macro;

use quote::quote;
use syn::{parse_macro_input, DeriveInput};

mod error;
mod replicate;

#[proc_macro_derive(Replicate, attributes(replicate))]
pub fn derive_replicate(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    replicate::derive(input)
        .unwrap_or_else(to_compile_errors)
        .into()
}

fn to_compile_errors(errors: Vec<syn::Error>) -> proc_macro2::TokenStream {
    let compile_errors = errors.iter().map(syn::Error::to_compile_error);
    quote!(#(#compile_errors)*)
}
