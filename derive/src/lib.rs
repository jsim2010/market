//! Adds derive macros for a market.
use {
    syn::{DeriveInput, parse_macro_input},
    quote::quote,
};


// Since unable to implement TryFrom<ConsumeFailure<T>> for T due to T not being covered, this macro implements that functionality.
/// Makes `item` able to be `T` in `ConsumeFailure<T>`.
#[inline]
#[proc_macro_derive(ConsumeFault)]
pub fn derive_consume_fault(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let output = quote! {
        #[allow(unused_qualifications)] // Macro does not know context of where it is being called.
        impl #impl_generics core::convert::TryFrom<market::ConsumeFailure<#ident #ty_generics>> for #ident #ty_generics #where_clause {
            type Error = ();

            #[inline]
            #[fehler::throws(())]
            fn try_from(failure: market::ConsumeFailure<Self>) -> Self {
                if let market::ConsumeFailure::Fault(fault) = failure {
                    fault
                } else {
                    fehler::throw!(())
                }
            }
        }
    };

    output.into()
}

// Since unable to implement TryFrom<ProduceFailure<T>> for T due to T not being covered, this macro implements that functionality.
/// Makes `item` able to be `T` in `ProduceFailure<T>`.
#[inline]
#[proc_macro_derive(ProduceFault)]
pub fn derive_produce_fault(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let output = quote! {
        #[allow(unused_qualifications)] // Macro does not know context of where it is being called.
        impl #impl_generics core::convert::TryFrom<market::ProduceFailure<#ident #ty_generics>> for #ident #ty_generics #where_clause {
            type Error = ();

            #[inline]
            #[fehler::throws(())]
            fn try_from(failure: market::ProduceFailure<Self>) -> Self {
                if let market::ProduceFailure::Fault(fault) = failure {
                    fault
                } else {
                    fehler::throw!(())
                }
            }
        }
    };

    output.into()
}
