/// Procedural macros for sysgen.
///
/// This crate is a placeholder. Macros will be added as the SysGen
/// toolchain grows.
use proc_macro::TokenStream;

/// Placeholder derive macro — will be implemented in a future issue.
#[proc_macro_derive(SysmlElement)]
pub fn derive_sysml_element(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
