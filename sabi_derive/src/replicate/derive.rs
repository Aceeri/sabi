use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{DeriveInput};

use crate::replicate::{attr, Ctxt};

pub fn derive(input: DeriveInput) -> Result<TokenStream, Vec<syn::Error>> {
    let mut base_ident = input.ident.clone();

    let ctxt = Ctxt::new();
    let attr = attr::Container::from_ast(&ctxt, &input);
    ctxt.check()?;

    let mut def = quote! { Self };
    let mut into_def = quote! { self };
    let mut from_def = quote! { def };

    let remote = if let Some(remote_path) = attr.remote {
        let last_segment = remote_path.segments.last().unwrap();
        base_ident = last_segment.ident.clone();

        let replicate_ident = Ident::new(&format!("Replicate{}", base_ident), Span::call_site());

        let remote_ident = &input.ident;
        let remote_ident_str = remote_ident.to_string();

        def = quote! { #replicate_ident };
        into_def = quote! { #replicate_ident(self) };
        from_def = quote! { def.0 };

        Some(quote! {
            #[derive(Debug, Clone, Serialize, Deserialize)]
            pub struct #replicate_ident(#[serde(with = #remote_ident_str)] pub #remote_path);
        })
    } else {
        None
    };

    let (sabi_path, sabi_crate) = match attr.sabi_path {
        Some(path) => (quote! { #path }, None),
        None => (
            quote! { _sabi },
            Some(quote! {
                #[allow(unused_extern_crates,clippy::useless::attribute)]
                extern crate sabi as _sabi;
            }),
        ),
    };

    Ok(quote! {
        #remote

        #[doc(hidden)]
        #[allow(non_upper_case_globals,unused_attributes)]
        const _: () = {
            #sabi_crate

            impl #sabi_path::Replicate for #base_ident {
                type Def = #def;
                fn into_def(self) -> Self::Def {
                    #into_def
                }
                fn from_def(def: Self::Def) -> Self {
                    #from_def
                }
            }
        };
    })
}
