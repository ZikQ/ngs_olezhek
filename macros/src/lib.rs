mod embed;
mod lua_type;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Attribute, Error, FnArg, Ident, ItemFn, LitStr, Pat, PatIdent, Path, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
};

const METHODS: &[&str] = &[
    "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "TRACE",
];

macro_rules! method_attrs {
    ($($name:ident => $method:literal),* $(,)?) => {
        $(
            #[proc_macro_attribute]
            pub fn $name(attr: TokenStream, item: TokenStream) -> TokenStream {
                expand(Some($method), attr, item)
            }
        )*
    };
}

method_attrs! {
    get => "GET",
    post => "POST",
    put => "PUT",
    delete => "DELETE",
    patch => "PATCH",
    head => "HEAD",
    options => "OPTIONS",
    trace => "TRACE",
}

/// `#[route("/path", GET, POST)]` для нескольких методов на один обработчик.
#[proc_macro_attribute]
pub fn route(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(None, attr, item)
}

/// `include_lua!("lua")` компилирует папку (путь от Cargo.toml крейта) в байткод и вшивает в бинарник.
/// Синтаксическая ошибка в Lua ломает сборку. Новый файл подхватится при следующем изменении любого
/// из уже вшитых или Rust-кода: за самой папкой cargo следить не умеет.
#[proc_macro]
pub fn include_lua(input: TokenStream) -> TokenStream {
    embed::include_lua(input.into()).unwrap_or_else(|e| e.to_compile_error()).into()
}

/// LuaLS-аннотации (`---@class`, `---@alias`) для JSON-формы типа с учётом атрибутов serde.
/// В dev-режиме они попадают в `<lua>/.ngs/api.lua`. `#[lua(type = "...")]` задаёт тип поля руками,
/// `#[lua(skip)]` прячет поле, `#[lua(optional)]` помечает его необязательным.
#[proc_macro_derive(LuaType, attributes(lua, serde))]
pub fn derive_lua_type(input: TokenStream) -> TokenStream {
    lua_type::derive(input.into()).unwrap_or_else(|e| e.to_compile_error()).into()
}

/// `routes![a, b, users::c]` собирает `axum::Router` из обработчиков с `#[get]`/`#[post]`/...
#[proc_macro]
pub fn routes(input: TokenStream) -> TokenStream {
    let handlers = parse_macro_input!(input with Punctuated::<Path, Token![,]>::parse_terminated);
    let routes = handlers.iter().map(|h| {
        quote! { .route(#h::PATH, ::axum::routing::on(#h::METHOD, #h)) }
    });
    quote!(::axum::Router::new() #(#routes)*).into()
}

struct RouteArgs {
    path: LitStr,
    methods: Vec<Ident>,
}

impl Parse for RouteArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;
        let mut methods = Vec::new();
        if input.parse::<Option<Token![,]>>()?.is_some() {
            methods = Punctuated::<Ident, Token![,]>::parse_terminated(input)?
                .into_iter()
                .collect();
        }
        Ok(Self { path, methods })
    }
}

fn expand(fixed: Option<&str>, attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = TokenStream2::from(item);
    match try_expand(fixed, attr.into(), item.clone()) {
        Ok(out) => out.into(),
        Err(err) => {
            let err = err.to_compile_error();
            quote!(#err #item).into()
        }
    }
}

fn try_expand(
    fixed: Option<&str>,
    attr: TokenStream2,
    item: TokenStream2,
) -> syn::Result<TokenStream2> {
    let args: RouteArgs = syn::parse2(attr)?;
    let mut func: ItemFn = syn::parse2(item)?;

    let filter = method_filter(fixed, &args)?;
    let params = parse_path_params(&args.path)?;
    let query = bind_params(&mut func, &params)?;

    let vis = &func.vis;
    let name = &func.sig.ident;
    let path = &args.path;

    let query_struct = (!query.is_empty()).then(|| {
        let fields = query.iter().map(|f| {
            let (ident, ty) = (&f.ident, &f.ty);
            quote!(pub #ident: #ty)
        });
        quote! {
            #[allow(unused_imports)]
            use super::*;

            #[derive(::serde::Deserialize)]
            pub struct QueryParams {
                #(#fields,)*
            }
        }
    });

    // Модуль с именем функции лежит в type-namespace и не конфликтует с самой функцией:
    // так `routes!` достаёт PATH/METHOD по тому же пути, которым импортирован обработчик.
    Ok(quote! {
        #func

        #[doc(hidden)]
        #vis mod #name {
            pub const PATH: &str = #path;
            pub const METHOD: ::axum::routing::MethodFilter = #filter;
            #query_struct
        }
    })
}

fn method_filter(fixed: Option<&str>, args: &RouteArgs) -> syn::Result<TokenStream2> {
    let span = args.path.span();
    let names: Vec<Ident> = match fixed {
        Some(method) => {
            if let Some(extra) = args.methods.first() {
                return Err(Error::new(extra.span(), "unexpected argument, expected only the path"));
            }
            vec![Ident::new(method, span)]
        }
        None => {
            if args.methods.is_empty() {
                return Err(Error::new(span, "specify at least one HTTP method: #[route(\"/path\", GET)]"));
            }
            args.methods
                .iter()
                .map(|m| {
                    let upper = m.to_string().to_uppercase();
                    if METHODS.contains(&upper.as_str()) {
                        Ok(Ident::new(&upper, m.span()))
                    } else {
                        Err(Error::new(m.span(), format!("unknown HTTP method, expected one of {METHODS:?}")))
                    }
                })
                .collect::<syn::Result<_>>()?
        }
    };

    let mut filters = names.iter().map(|m| quote!(::axum::routing::MethodFilter::#m));
    let first = filters.next().expect("at least one method");
    Ok(filters.fold(first, |acc, next| quote!(#acc.or(#next))))
}

/// Имена параметров в порядке появления в пути. `{*rest}` даёт `rest`, `{{`/`}}` экранируют скобки.
fn parse_path_params(lit: &LitStr) -> syn::Result<Vec<String>> {
    let path = lit.value();
    if !path.starts_with('/') {
        return Err(Error::new(lit.span(), "route path must start with '/'"));
    }

    let mut params: Vec<String> = Vec::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
            }
            '}' => return Err(Error::new(lit.span(), "unmatched '}' in route path, use '}}' to escape")),
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(ch) => name.push(ch),
                        None => return Err(Error::new(lit.span(), "unclosed '{' in route path")),
                    }
                }
                let name = name.strip_prefix('*').unwrap_or(&name).to_string();
                if name.is_empty() {
                    return Err(Error::new(lit.span(), "empty parameter name in route path"));
                }
                if params.contains(&name) {
                    return Err(Error::new(lit.span(), format!("duplicate path parameter `{name}`")));
                }
                params.push(name);
            }
            _ => {}
        }
    }
    Ok(params)
}

/// Аргументы функции, чьё имя совпало с `{param}`, переезжают в один `Path<(..)>` в порядке пути.
/// Параметры без аргумента получают `_: String`, потому что axum требует точное число элементов.
/// Если совпадений нет, `Path` не добавляется: значит, он (если нужен) написан руками.
/// Аргументы с `#[query]` собираются в `Query<QueryParams>`; сама структура создаётся в `try_expand`.
fn bind_params(func: &mut ItemFn, params: &[String]) -> syn::Result<Vec<QueryField>> {
    let mut slots: Vec<Option<(Pat, Type)>> = vec![None; params.len()];
    let mut query = Vec::new();
    let mut rest = Punctuated::<FnArg, Token![,]>::new();

    for mut arg in std::mem::take(&mut func.sig.inputs) {
        let span = arg.span();
        let FnArg::Typed(typed) = &mut arg else {
            return Err(Error::new(span, "route handlers cannot take `self`"));
        };

        let is_query = take_query_attr(&mut typed.attrs)?;
        let plain_ident = match &*typed.pat {
            Pat::Ident(PatIdent { ident, by_ref: None, subpat: None, .. }) => Some(ident.clone()),
            _ => None,
        };

        if is_query {
            let Some(ident) = plain_ident else {
                return Err(Error::new(typed.pat.span(), "#[query] needs a plain identifier, e.g. `page: u32`"));
            };
            query.push(QueryField { ident, pat: (*typed.pat).clone(), ty: (*typed.ty).clone() });
            continue;
        }

        if let Some(ident) = plain_ident
            && let Some(i) = params.iter().position(|p| ident == p)
        {
            if slots[i].is_some() {
                return Err(Error::new(ident.span(), format!("duplicate argument `{ident}`")));
            }
            slots[i] = Some(((*typed.pat).clone(), (*typed.ty).clone()));
            continue;
        }
        rest.push(arg);
    }

    let mut extractors: Vec<FnArg> = Vec::new();

    if slots.iter().any(Option::is_some) {
        let pats = slots.iter().map(|s| match s {
            Some((pat, _)) => quote!(#pat),
            None => quote!(_),
        });
        let tys = slots.iter().map(|s| match s {
            Some((_, ty)) => quote!(#ty),
            None => quote!(::std::string::String),
        });
        extractors.push(parse_quote! {
            ::axum::extract::Path((#(#pats,)*)): ::axum::extract::Path<(#(#tys,)*)>
        });
    }

    if !query.is_empty() {
        let module = &func.sig.ident;
        let fields = query.iter().map(|f| &f.pat);
        extractors.push(parse_quote! {
            ::axum::extract::Query(#module::QueryParams { #(#fields),* }): ::axum::extract::Query<#module::QueryParams>
        });
    }

    func.sig.inputs = extractors.into_iter().chain(rest).collect();
    Ok(query)
}

struct QueryField {
    ident: Ident,
    pat: Pat,
    ty: Type,
}

fn take_query_attr(attrs: &mut Vec<Attribute>) -> syn::Result<bool> {
    let Some(i) = attrs.iter().position(|a| a.path().is_ident("query")) else {
        return Ok(false);
    };
    attrs.remove(i).meta.require_path_only()?;
    Ok(true)
}
