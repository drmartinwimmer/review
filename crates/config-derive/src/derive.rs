use quote::quote;
use syn::{DeriveInput, Error, LitStr};

pub(crate) fn impl_config_file(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut config_file_path: Option<LitStr> = None;

    for attr in &input.attrs {
        if attr.path().is_ident("config_file") {
            let lit: LitStr = attr.parse_args()?;
            config_file_path = Some(lit);
        }
    }

    let Some(path_lit) = config_file_path else {
        return Err(Error::new_spanned(
            input,
            "derive(ConfigFile) requires a #[config_file(\"path/to/file\")] attribute",
        ));
    };

    let krate = match std::env::var("CARGO_PKG_NAME").as_deref() {
        Ok("code-review-config") => quote!(crate),
        _ => quote!(::code_review_config),
    };

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #krate::ConfigFile for #name #ty_generics #where_clause {
            fn config_file_path() -> &'static str {
                #path_lit
            }
        }
    })
}
