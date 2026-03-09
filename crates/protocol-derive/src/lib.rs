//! Derive macro for generating schema metadata from Rust type definitions.
//!
//! Annotate protocol enums and structs with `#[derive(HasSchema)]` to
//! automatically generate `SchemaType` descriptors. This eliminates manual
//! schema maintenance — the schema always matches the actual type definition.
//!
//! Uses `crate::` paths in generated code, so the macro must be used within
//! the `protocol` crate (or re-exported from it with a `use` alias).

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// Derive the `HasSchema` trait, generating a `schema()` method that returns
/// a `SchemaType` describing this type's structure.
///
/// Works on enums (with named, unnamed, or unit variants) and structs.
#[proc_macro_derive(HasSchema)]
pub fn derive_has_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();

    let (kind, variants) = match &input.data {
        Data::Enum(data) => {
            let variants = data.variants.iter().map(|v| {
                let variant_name = v.ident.to_string();
                let fields = schema_fields(&v.fields);
                quote! {
                    crate::SchemaVariant {
                        name: #variant_name.into(),
                        fields: vec![#(#fields),*],
                    }
                }
            });
            ("enum", quote! { vec![#(#variants),*] })
        }
        Data::Struct(data) => {
            let fields = schema_fields(&data.fields);
            let single_variant = quote! {
                crate::SchemaVariant {
                    name: #name_str.into(),
                    fields: vec![#(#fields),*],
                }
            };
            ("struct", quote! { vec![#single_variant] })
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(name, "HasSchema cannot be derived for unions")
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        impl crate::HasSchema for #name {
            fn schema() -> crate::SchemaType {
                crate::SchemaType {
                    name: #name_str.into(),
                    kind: #kind.into(),
                    variants: #variants,
                }
            }
        }
    };

    expanded.into()
}

/// Generate `SchemaField` tokens for a set of fields.
fn schema_fields(fields: &Fields) -> Vec<proc_macro2::TokenStream> {
    match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|f| {
                let field_name = f.ident.as_ref().unwrap().to_string();
                let field_ty = type_to_string(&f.ty);
                quote! {
                    crate::SchemaField {
                        name: #field_name.into(),
                        ty: #field_ty.into(),
                    }
                }
            })
            .collect(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let field_name = i.to_string();
                let field_ty = type_to_string(&f.ty);
                quote! {
                    crate::SchemaField {
                        name: #field_name.into(),
                        ty: #field_ty.into(),
                    }
                }
            })
            .collect(),
        Fields::Unit => Vec::new(),
    }
}

/// Render a type as a string token. Uses the source representation so
/// `Vec<u8>` becomes `"Vec<u8>"`, `Option<String>` becomes `"Option<String>"`.
fn type_to_string(ty: &syn::Type) -> String {
    quote!(#ty).to_string().replace(" ", "")
}
