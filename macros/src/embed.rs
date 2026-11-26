use std::{env, fs, path::PathBuf};

use ngs_olezhek_bundle::{Entry, collect};
use omnilua::Lua;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Error, LitByteStr, LitStr};

pub fn include_lua(input: TokenStream) -> syn::Result<TokenStream> {
    let lit: LitStr = syn::parse2(input)?;
    let span = lit.span();
    let manifest = env::var("CARGO_MANIFEST_DIR").map_err(|_| Error::new(span, "CARGO_MANIFEST_DIR is not set"))?;
    let dir = PathBuf::from(manifest).join(lit.value());
    let files = collect(&dir).map_err(|e| Error::new(span, format!("lua dir {}: {e}", dir.display())))?;

    let lua = Lua::new();
    let mut tracked = Vec::new();
    let mut chunks = Vec::new();
    let mut errors: Option<Error> = None;
    for (name, path) in &files {
        let source = fs::read(path).map_err(|e| Error::new(span, format!("{}: {e}", path.display())))?;
        // include_bytes! puts the file into rustc's dep-info, so editing it re-runs this macro.
        // The constant is never used, so the source itself doesn't end up in the binary.
        let path = path.to_string_lossy().into_owned();
        tracked.push(quote!(const _: &[u8] = include_bytes!(#path);));

        let entry = Entry::compile(&lua, name, &source, false);
        if let Some(msg) = entry.error() {
            let err = Error::new(span, format!("lua: {msg}"));
            match &mut errors {
                Some(all) => all.combine(err),
                None => errors = Some(err),
            }
            continue;
        }
        let bytes = LitByteStr::new(&entry.data, span);
        chunks.push(quote!((#name, #bytes as &[u8])));
    }
    if let Some(err) = errors {
        return Err(err);
    }

    let dir = dir.to_string_lossy().into_owned();
    Ok(quote! {{
        #(#tracked)*
        ::ngs_olezhek::EmbeddedLua::from_parts(#dir, &[#(#chunks),*])
    }})
}
