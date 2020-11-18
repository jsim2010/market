extern crate proc_macro;

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, DeriveInput},
};

// Since unable to implement TryFrom<ConsumeFailure<T>> for T due to T not being covered, this macro implements that functionality.
/// Makes type able to be T in ConsumeFailure<T>.
#[proc_macro_derive(ConsumeFault)]
pub fn derive_consume_fault(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let output = quote! {
        #[allow(unused_qualifications)]
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
/// Makes type able to be T in ProduceFailure<T>.
#[proc_macro_derive(ProduceFault)]
pub fn derive_produce_fault(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let output = quote! {
        #[allow(unused_qualifications)]
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
