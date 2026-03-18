// Traceability attribute macros for sysgen.
//
// Approach: Option B — doc-attribute encoding.
// Each macro emits one `#[doc = "sysgen:<kind>:<req_id>"]` attribute per
// requirement ID.  The `sysgen-trace` crate walks source ASTs with `syn` and
// finds these attributes at check time.  No `linkme` or circular-dependency
// issues arise because the macros themselves carry no runtime registration.
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, Item, LitStr, Token};

// Parse a comma-separated list of string literals.
struct ReqIds {
    ids: Vec<LitStr>,
}

impl syn::parse::Parse for ReqIds {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ids = Punctuated::<LitStr, Token![,]>::parse_terminated(input)?;
        Ok(ReqIds {
            ids: ids.into_iter().collect(),
        })
    }
}

/// Mark an item as implementing one or more SysML v2 requirements.
///
/// # Examples
/// ```ignore
/// #[implements("VehicleRequirements::MassRequirement")]
/// pub fn check_mass(vehicle: &Vehicle) -> bool { ... }
///
/// #[implements("R1", "R2")]
/// pub struct VehicleSafetySystem { ... }
/// ```
#[proc_macro_attribute]
pub fn implements(attr: TokenStream, item: TokenStream) -> TokenStream {
    let req_ids = parse_macro_input!(attr as ReqIds);
    let item = parse_macro_input!(item as Item);

    let doc_attrs = doc_attrs_for(&req_ids.ids, "implements");

    quote! {
        #(#doc_attrs)*
        #item
    }
    .into()
}

/// Mark a test function as verifying one or more SysML v2 requirements.
///
/// # Examples
/// ```ignore
/// #[verifies("VehicleRequirements::MassRequirement")]
/// #[test]
/// fn test_mass_under_limit() { ... }
/// ```
#[proc_macro_attribute]
pub fn verifies(attr: TokenStream, item: TokenStream) -> TokenStream {
    let req_ids = parse_macro_input!(attr as ReqIds);
    let item = parse_macro_input!(item as Item);

    let doc_attrs = doc_attrs_for(&req_ids.ids, "verifies");

    quote! {
        #(#doc_attrs)*
        #item
    }
    .into()
}

fn doc_attrs_for(ids: &[LitStr], kind: &str) -> Vec<proc_macro2::TokenStream> {
    ids.iter()
        .map(|id| {
            let doc_value = format!("sysgen:{}:{}", kind, id.value());
            quote! { #[doc = #doc_value] }
        })
        .collect()
}

/// Placeholder derive macro — will be implemented in a future issue.
#[proc_macro_derive(SysmlElement)]
pub fn derive_sysml_element(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
