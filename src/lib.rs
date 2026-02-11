//! Generate `todo!()` stubs in testing mode for trait methods without default implementations.
//!
//! # Overview
//!
//! This library provides a proc macro attribute `test_stubs` which can be attached to traits: for
//! each method in the trait without a default implementation, two variants will be created, one
//! for `#[cfg(not(test))]` and one for `#[cfg(test)]`. The latter will have a stubbed method body
//! containing just `todo!("<method name>")`, allowing tests to implement the trait without having
//! to manually implement each method. If that method is then called, it will `todo` and tell the
//! user which method needs to be implemented.
//!
//! Roughly speaking, given the following Rust source file:
//!
//! ```text
//! #[test_stubs]
//! trait T {
//!   fn f(&self) { ... }
//!   fn g(&self);
//! }
//! ```
//!
//! will produce:
//!
//! ```text
//! trait T {
//!   fn f(&self) { ... }
//!
//!   #[cfg(not(test))]
//!   fn g(&self);
//!
//!   #[cfg(test)]
//!   fn g(&self) { todo!("g") }
//! }
//! ```
//!
//! Note: `f` was copied over unchanged, but two copies of `g` were generated, one with and one
//! without a default implementation.
//!
//!
//! ## Limitations
//!
//! There are limitation to what `test_stubs` can do.
//!
//! For example, Rust's type inference isn't always happy with just `todo!()`. This code will not
//! compile:
//!
//! ```text
//! trait T {
//!   #[cfg(test)]
//!   fn f() -> impl Iterator<...> { todo!("f") }
//! }
//! ```
//!
//! There is no generic solution to this. `test_stubs` knows about some common types and will
//! generate code for them. For a type such as the above it will generate:
//!
//! ```text
//! trait T {
//!   #[cfg(test)]
//!   fn f() -> impl Iterator<...> { todo!("f") as std::iter::Empty<_> }
//! }
//! ```
//!
//! When `test_stubs` has no specific knowledge about a type, it will simply generate `todo!()` and
//! hope.
//!
//! If a trait method takes `self` (rather than `&self`), `test_stubs` will add a `where Self:
//! Sized` constraint to the `#[cfg(test)]` method.
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    FnArg, GenericArgument, ItemTrait, Meta, PathArguments, ReturnType, TraitItem, Type,
    TypeImplTrait, TypeParamBound, WherePredicate, parse_macro_input,
};

#[proc_macro_attribute]
pub fn test_stubs(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut trait_item = parse_macro_input!(item as ItemTrait);

    // rustc complains that the trait we attach to is unused, so silence it by attaching
    // `unreachable_code` to the trait.
    trait_item
        .attrs
        .push(syn::parse_quote!(#[allow(unreachable_code)]));

    let mut new_items = Vec::with_capacity(trait_item.items.len());
    for item in trait_item.items.into_iter() {
        match item {
            TraitItem::Fn(mut meth) if meth.default.is_none() => {
                // If the method is already `#[cfg(test)]`, there is nothing to duplicate.
                if meth.attrs.iter().any(|x| {
                    x.path().is_ident("cfg")
                        && matches!(&x.meta, Meta::List(y) if y.path.is_ident("test"))
                }) {
                    new_items.push(TraitItem::Fn(meth));
                    continue;
                }

                // The `#[cfg(not(test))]` variant.
                let mut not_test = meth.clone();
                not_test.attrs.push(syn::parse_quote!(#[cfg(not(test))]));
                new_items.push(TraitItem::Fn(not_test));

                // The `#[cfg(test)]` variant.
                meth.attrs.push(syn::parse_quote!(#[cfg(test)]));

                // Silence warnings about unused parameters.
                meth.attrs
                    .push(syn::parse_quote!(#[allow(unused_variables)]));
                // Silence warnings about `todo!()` being unusable code.
                meth.attrs
                    .push(syn::parse_quote!(#[allow(unreachable_code)]));

                // If the self type is `self`, we have to ensure `where Self: Sized` is part of the
                // `where` predicates.
                if matches!(
                    meth.sig.inputs.first(),
                    Some(FnArg::Receiver(recv)) if recv.reference.is_none()
                ) {
                    let wheres = meth.sig.generics.make_where_clause();
                    // Search for `where Self:Sized`, adding it if not present.
                    if !wheres.predicates.iter().any(is_self_sized_pred) {
                        wheres.predicates.push(syn::parse_quote!(Self: Sized));
                    }
                }

                let name = meth.sig.ident.to_string();
                let stubexpr = match &meth.sig.output {
                    ReturnType::Default => {
                        quote! { todo!(#name) }
                    }
                    ReturnType::Type(_, ty) => stub_expr_for_ty(ty, &name),
                };
                meth.default = Some(syn::parse_quote!({ #stubexpr }));

                new_items.push(TraitItem::Fn(meth));
            }
            x => new_items.push(x),
        }
    }

    trait_item.items = new_items;
    TokenStream::from(quote!(#trait_item))
}

/// Return `true` if this [WherePredicate] is `Self: Sized`.
fn is_self_sized_pred(pred: &WherePredicate) -> bool {
    if let WherePredicate::Type(ty) = pred
        && let Type::Path(p) = &ty.bounded_ty
        && p.qself.is_none()
        && p.path.segments.len() == 1
        && p.path.segments[0].ident == "Self"
        && ty
            .bounds
            .iter()
            .any(|y| matches!(y, TypeParamBound::Trait(t) if t.path.is_ident("Sized")))
    {
        true
    } else {
        false
    }
}

/// Recursively generate a stub expression for a type `ty` in method `name`. For example for:
/// ```text
/// (u32, impl Iterator<...>, Option<impl Iterator<...>>)
/// ```
/// this will create a stub along the lines of:
/// ```text
/// (
///   todo!("<name>"),
///   todo!("<name>") as std::iter::Empty<_>,
///   Some(todo!("<name>") as std::iter::Empty<_>)
/// ```
///
/// As that suggests, this method special cases certain types. When
fn stub_expr_for_ty(ty: &Type, name: &str) -> proc_macro2::TokenStream {
    match ty {
        Type::ImplTrait(TypeImplTrait { bounds, .. }) => {
            // Just `todo!()` for a type `impl X` doesn't work.
            if bounds.iter().any(|x| {
                matches!(x, TypeParamBound::Trait(t) if t.path.segments.last().unwrap().ident == "Iterator")
            }) {
                quote! { todo!(#name) as std::iter::Empty<_> }
            } else {
                // What can we do for arbitrary `impl` types? Just outputting `todo!()` is unlikely
                // to satisfy type inference.
                quote! { todo!(#name) }
            }
        }
        Type::Path(ty_p) => {
            let last = ty_p.path.segments.last().unwrap();
            match &last.arguments {
                PathArguments::AngleBracketed(args) => {
                    let outerty = args
                        .args
                        .iter()
                        .find_map(|arg| match arg {
                            GenericArgument::Type(ty) => Some(ty),
                            _ => None,
                        })
                        .unwrap();
                    let stub = stub_expr_for_ty(outerty, name);
                    // We special case certain common types where we are easily able to create
                    // expressions / variants that, even with deeply nested types, will satisfy
                    // type inference.
                    match last.ident.to_string().as_str() {
                        "Box" => quote! { Box::new(#stub) },
                        "Option" => quote! { Some(#stub) },
                        "Result" => quote! { Ok(#stub) },
                        _ => quote! { todo!(#name) },
                    }
                }
                _ => quote! { todo!(#name) },
            }
        }
        Type::Tuple(x) => {
            let elems: Vec<_> = x.elems.iter().map(|x| stub_expr_for_ty(x, name)).collect();
            quote! { (#(#elems),*) }
        }
        _ => quote! { todo!(#name) },
    }
}
