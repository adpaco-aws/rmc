// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! This module provides Kani's `derive` macro for `Arbitrary`.
//!
//! ```
//! use kani::Arbitrary;
//!
//! #[derive(Arbitrary)]
//! struct S;
//!
//! ```
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::abort;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, Data, DataEnum, DeriveInput, Fields, GenericParam, Generics,
    Index,
};

pub fn expand_derive_arbitrary(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derive_item = parse_macro_input!(item as DeriveInput);
    let item_name = &derive_item.ident;

    // Add a bound `T: Arbitrary` to every type parameter T.
    let generics = add_trait_bound_arbitrary(derive_item.generics);
    // Generate an expression to sum up the heap size of each field.
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = fn_any_body(&item_name, &derive_item.data);

    // Get the safety constraints (if any) to produce type-safe values
    let safety_conds_opt = safety_conds(&item_name, &derive_item.data);

    let expanded = if let Some(safety_cond) = safety_conds_opt {
        let field_refs = field_refs(&item_name, &derive_item.data);
        quote! {
            // The generated implementation.
            impl #impl_generics kani::Arbitrary for #item_name #ty_generics #where_clause {
                fn any() -> Self {
                    let obj = #body;
                    #field_refs
                    kani::assume(#safety_cond);
                    obj
                }
            }
        }
    } else {
        quote! {
            // The generated implementation.
            impl #impl_generics kani::Arbitrary for #item_name #ty_generics #where_clause {
                fn any() -> Self {
                    #body
                }
            }
        }
    };
    proc_macro::TokenStream::from(expanded)
}

/// Add a bound `T: Arbitrary` to every type parameter T.
fn add_trait_bound_arbitrary(mut generics: Generics) -> Generics {
    generics.params.iter_mut().for_each(|param| {
        if let GenericParam::Type(type_param) = param {
            type_param.bounds.push(parse_quote!(kani::Arbitrary));
        }
    });
    generics
}

/// Generate the body of the function `any()`.
/// This will create the non-deterministic object.
/// E.g.:
/// ```
/// #[derive(Arbitrary)]
/// struct Point { x: u8, y: u8 }
/// ```
/// will generate the following body for `fn any()`:
/// ```
/// fn any() -> Self {
///    Self { x: kani::any(), y: kani::any() }
/// }
/// ```
pub fn fn_any_body(ident: &Ident, data: &Data) -> TokenStream {
    match data {
        Data::Struct(struct_data) => init_symbolic_item(ident, &struct_data.fields),
        Data::Enum(enum_data) => fn_any_enum(ident, enum_data),
        Data::Union(_) => {
            abort!(Span::call_site(), "Cannot derive `Arbitrary` for `{}` union", ident;
                note = ident.span() =>
                "`#[derive(Arbitrary)]` cannot be used for unions such as `{}`", ident
            )
        }
    }
}

/// Parse the condition expressions in `#[safety_constraint(<cond>)]` attached to struct
/// fields and, it at least one was found, generate a conjunction to be assumed.
///
/// For example, if we're deriving implementations for the struct
/// ```
/// #[derive(Arbitrary)]
/// #[derive(Invariant)]
/// struct PositivePoint {
///     #[safety_constraint(*x >= 0)]
///     x: i32,
///     #[safety_constraint(*y >= 0)]
///     y: i32,
/// }
/// ```
/// this function will generate the `TokenStream`
/// ```
/// *x >= 0 && *y >= 0
/// ```
/// which can be passed to `kani::assume` to constrain the values generated
/// through the `Arbitrary` impl so that they are type-safe by construction.
fn safety_conds(ident: &Ident, data: &Data) -> Option<TokenStream> {
    match data {
        Data::Struct(struct_data) => safety_conds_inner(ident, &struct_data.fields),
        Data::Enum(_) => None,
        Data::Union(_) => None,
    }
}

/// Generates an expression resulting from the conjunction of conditions
/// specified as safety constraints for each field. See `safety_conds` for more details.
fn safety_conds_inner(ident: &Ident, fields: &Fields) -> Option<TokenStream> {
    match fields {
        Fields::Named(ref fields) => {
            let conds: Vec<TokenStream> =
                fields.named.iter().filter_map(|field| parse_safety_expr(ident, field)).collect();
            if !conds.is_empty() { Some(quote! { #(#conds)&&* }) } else { None }
        }
        Fields::Unnamed(_) => None,
        Fields::Unit => None,
    }
}

/// Generates the sequence of expressions to initialize the variables used as
/// references to the struct fields.
///
/// For example, if we're deriving implementations for the struct
/// ```
/// #[derive(Arbitrary)]
/// #[derive(Invariant)]
/// struct PositivePoint {
///     #[safety_constraint(*x >= 0)]
///     x: i32,
///     #[safety_constraint(*y >= 0)]
///     y: i32,
/// }
/// ```
/// this function will generate the `TokenStream`
/// ```
/// let x = &obj.x;
/// let y = &obj.y;
/// ```
/// which allows us to refer to the struct fields without using `self`.
/// Note that the actual stream is generated in the `field_refs_inner` function.
pub fn field_refs(ident: &Ident, data: &Data) -> TokenStream {
    match data {
        Data::Struct(struct_data) => field_refs_inner(ident, &struct_data.fields),
        Data::Enum(_) => unreachable!(),
        Data::Union(_) => unreachable!(),
    }
}

/// Generates the sequence of expressions to initialize the variables used as
/// references to the struct fields. See `field_refs` for more details.
fn field_refs_inner(_ident: &Ident, fields: &Fields) -> TokenStream {
    match fields {
        Fields::Named(ref fields) => {
            let field_refs: Vec<TokenStream> = fields
                .named
                .iter()
                .map(|field| {
                    let name = &field.ident;
                    quote_spanned! {field.span()=>
                        let #name = &obj.#name;
                    }
                })
                .collect();
            if !field_refs.is_empty() {
                quote! { #( #field_refs )* }
            } else {
                quote! {}
            }
        }
        Fields::Unnamed(_) => quote! {},
        Fields::Unit => quote! {},
    }
}

pub fn safe_body_default(ident: &Ident, data: &Data) -> TokenStream {
    match data {
        Data::Struct(struct_data) => safe_body_default_inner(ident, &struct_data.fields),
        Data::Enum(_) => unreachable!(),
        Data::Union(_) => unreachable!(),
    }
}

fn safe_body_default_inner(_ident: &Ident, fields: &Fields) -> TokenStream {
    match fields {
        Fields::Named(ref fields) => {
            let field_safe_calls: Vec<TokenStream> = fields
                .named
                .iter()
                .map(|field| {
                    let name = &field.ident;
                    quote_spanned! {field.span()=>
                        #name.is_safe()
                    }
                })
                .collect();
            if !field_safe_calls.is_empty() {
                quote! { #( #field_safe_calls )&&* }
            } else {
                quote! { true }
            }
        }
        Fields::Unnamed(_) => quote! {},
        Fields::Unit => quote! {},
    }
}

/// Generate an item initialization where an item can be a struct or a variant.
/// For named fields, this will generate: `Item { field1: kani::any(), field2: kani::any(), .. }`
/// For unnamed fields, this will generate: `Item (kani::any(), kani::any(), ..)`
/// For unit field, generate an empty initialization.
fn init_symbolic_item(ident: &Ident, fields: &Fields) -> TokenStream {
    match fields {
        Fields::Named(ref fields) => {
            // Use the span of each `syn::Field`. This way if one of the field types does not
            // implement `Arbitrary` then the compiler's error message underlines which field it
            // is. An example is shown in the readme of the parent directory.
            let init = fields.named.iter().map(|field| {
                let name = &field.ident;
                quote_spanned! {field.span()=>
                    #name: kani::any()
                }
            });
            quote! {
                #ident {#( #init,)*}
            }
        }
        Fields::Unnamed(ref fields) => {
            // Expands to an expression like
            // Self(kani::any(), kani::any(), ..., kani::any());
            let init = fields.unnamed.iter().map(|field| {
                quote_spanned! {field.span()=>
                    kani::any()
                }
            });
            quote! {
                #ident(#( #init,)*)
            }
        }
        Fields::Unit => {
            quote! {
                #ident
            }
        }
    }
}

/// Extract, parse and return the expression `cond` (i.e., `Some(cond)`) in the
/// `#[safety_constraint(<cond>)]` attribute helper associated with a given field.
/// Return `None` if the attribute isn't specified.
fn parse_safety_expr(ident: &Ident, field: &syn::Field) -> Option<TokenStream> {
    let name = &field.ident;
    let mut safety_helper_attr = None;

    // Keep the helper attribute if we find it
    for attr in &field.attrs {
        if attr.path().is_ident("safety_constraint") {
            safety_helper_attr = Some(attr);
        }
    }

    // Parse the arguments in the `#[safety_constraint(...)]` attribute
    if let Some(attr) = safety_helper_attr {
        let expr_args: Result<syn::Expr, syn::Error> = attr.parse_args();

        // Check if there was an error parsing the arguments
        if let Err(err) = expr_args {
            abort!(Span::call_site(), "Cannot derive impl for `{}`", ident;
            note = attr.span() =>
            "safety constraint in field `{}` could not be parsed: {}", name.as_ref().unwrap().to_string(), err
            )
        }

        // Return the expression associated to the safety constraint
        let safety_expr = expr_args.unwrap();
        Some(quote_spanned! {field.span()=>
            #safety_expr
        })
    } else {
        None
    }
}

fn parse_safety_expr_struct(ident: &Ident, derive_input: &DeriveInput) -> Option<TokenStream> {
    let name = ident;
    let mut safety_attr = None;

    // Keep the attribute if we find it
    for attr in &derive_input.attrs {
        if attr.path().is_ident("safety_constraint") {
            safety_attr = Some(attr);
        }
    }

    // Parse the arguments in the `#[safety_constraint(...)]` attribute
    if let Some(attr) = safety_attr {
        let expr_args: Result<syn::Expr, syn::Error> = attr.parse_args();

        // Check if there was an error parsing the arguments
        if let Err(err) = expr_args {
            abort!(Span::call_site(), "Cannot derive impl for `{}`", ident;
            note = attr.span() =>
            "safety constraint in `{}` could not be parsed: {}", name.to_string(), err
            )
        }

        // Return the expression associated to the safety constraint
        let safety_expr = expr_args.unwrap();
        Some(quote_spanned! {derive_input.span()=>
            #safety_expr
        })
    } else {
        None
    }
}
/// Generate the body of the function `any()` for enums. The cases are:
/// 1. For zero-variants enumerations, this will encode a `panic!()` statement.
/// 2. For one or more variants, the code will be something like:
/// ```
/// # enum Enum{
/// #    WithoutData,
/// #    WithUnNamedData(i32),
/// #    WithNamedData{ i: i32},
/// # }
/// #
/// # impl kani::Arbitrary for Enum {
/// #     fn any() -> Self {
///         match kani::any() {
///             0 => Enum::WithoutData,
///             1 => Enum::WithUnNamedData(kani::any()),
///             _ => Enum::WithNamedData {i: kani::any()},
///         }
/// #    }
/// # }
/// ```
fn fn_any_enum(ident: &Ident, data: &DataEnum) -> TokenStream {
    if data.variants.is_empty() {
        let msg = format!(
            "Cannot create symbolic enum `{ident}`. Enums with zero-variants cannot be instantiated"
        );
        quote! {
            panic!(#msg)
        }
    } else {
        let arms = data.variants.iter().enumerate().map(|(idx, variant)| {
            let init = init_symbolic_item(&variant.ident, &variant.fields);
            if idx + 1 < data.variants.len() {
                let index = Index::from(idx);
                quote! {
                    #index => #ident::#init,
                }
            } else {
                quote! {
                    _ => #ident::#init,
                }
            }
        });

        quote! {
            match kani::any() {
                #(#arms)*
            }
        }
    }
}

pub fn expand_derive_invariant(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derive_item = parse_macro_input!(item as DeriveInput);
    let item_name = &derive_item.ident;

    let safe_body = safe_body(&item_name, &derive_item);
    let field_refs = field_refs(&item_name, &derive_item.data);

    // Add a bound `T: Invariant` to every type parameter T.
    let generics = add_trait_bound_invariant(derive_item.generics);
    // Generate an expression to sum up the heap size of each field.
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        // The generated implementation.
        impl #impl_generics kani::Invariant for #item_name #ty_generics #where_clause {
            fn is_safe(&self) -> bool {
                let obj = self;
                #field_refs
                #safe_body
            }
        }
    };
    proc_macro::TokenStream::from(expanded)
}

fn safe_body(item_name: &Ident, derive_input: &DeriveInput) -> TokenStream {
    let has_item_safety_constraint =
        derive_input.attrs.iter().any(|attr| attr.path().is_ident("safety_constraint"));
    let has_field_safety_constraints = has_field_safety_constraints(&item_name, &derive_input.data);

    if has_item_safety_constraint && has_field_safety_constraints {
        abort!(Span::call_site(), "Cannot derive `Invariant` for `{}`", item_name;
        note = item_name.span() =>
        "`#[safety_constaint(...)]` cannot be used in struct AND its fields"
        )
    }

    let safe_body_from_attrs_opt: Option<TokenStream> = if has_item_safety_constraint {
        Some(safe_body_from_struct_attr(&item_name, &derive_input))
    } else if has_field_safety_constraints {
        Some(safe_body_from_fields_attr(&item_name, &derive_input.data))
    } else {
        None
    };

    let safe_body_default = safe_body_default(&item_name, &derive_input.data);

    if let Some(safe_body_from_attrs) = safe_body_from_attrs_opt {
        quote! { #safe_body_default && #safe_body_from_attrs }
    } else {
        safe_body_default
    }
}

fn has_field_safety_constraints(ident: &Ident, data: &Data) -> bool {
    match data {
        Data::Struct(struct_data) => has_field_safety_constraints_inner(ident, &struct_data.fields),
        Data::Enum(_) => false,
        Data::Union(_) => false,
    }
}

fn has_field_safety_constraints_inner(_ident: &Ident, fields: &Fields) -> bool {
    match fields {
        Fields::Named(ref fields) => fields
            .named
            .iter()
            .any(|field| field.attrs.iter().any(|attr| attr.path().is_ident("safety_constraint"))),
        Fields::Unnamed(_) => false,
        Fields::Unit => false,
    }
}

/// Add a bound `T: Invariant` to every type parameter T.
pub fn add_trait_bound_invariant(mut generics: Generics) -> Generics {
    generics.params.iter_mut().for_each(|param| {
        if let GenericParam::Type(type_param) = param {
            type_param.bounds.push(parse_quote!(kani::Invariant));
        }
    });
    generics
}

fn safe_body_from_struct_attr(ident: &Ident, derive_input: &DeriveInput) -> TokenStream {
    parse_safety_expr_struct(ident, derive_input).unwrap()
}

fn safe_body_from_fields_attr(ident: &Ident, data: &Data) -> TokenStream {
    match data {
        Data::Struct(struct_data) => struct_invariant_conjunction(ident, &struct_data.fields),
        Data::Enum(_) => {
            abort!(Span::call_site(), "Cannot derive `Invariant` for `{}` enum", ident;
                note = ident.span() =>
                "`#[derive(Invariant)]` cannot be used for enums such as `{}`", ident
            )
        }
        Data::Union(_) => {
            abort!(Span::call_site(), "Cannot derive `Invariant` for `{}` union", ident;
                note = ident.span() =>
                "`#[derive(Invariant)]` cannot be used for unions such as `{}`", ident
            )
        }
    }
}

/// Generates an expression that is the conjunction of safety constraints for each field in the struct.
fn struct_invariant_conjunction(ident: &Ident, fields: &Fields) -> TokenStream {
    match fields {
        // Expands to the expression
        // `<safety_cond1> && <safety_cond2> && ..`
        // where `<safety_condN>` is the safety condition specified for the N-th field.
        Fields::Named(ref fields) => {
            let safety_conds: Vec<TokenStream> =
                fields.named.iter().filter_map(|field| parse_safety_expr(ident, field)).collect();
            quote! { #(#safety_conds)&&* }
        }
        Fields::Unnamed(_) => {
            quote! {
                true
            }
        }
        // Expands to the expression
        // `true`
        Fields::Unit => {
            quote! {
                true
            }
        }
    }
}
