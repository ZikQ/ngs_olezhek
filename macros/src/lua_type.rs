//! `#[derive(LuaType)]`: LuaLS annotations for a type's JSON shape, following serde's attributes.
//! The text is built at compile time and registered with `inventory`, so the app only has to
//! derive; dev mode writes the collected classes to `.ngs/api.lua`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Expr, ExprLit, Fields, GenericArgument, Lit, LitStr, Meta, PathArguments, Token, Type,
    meta::ParseNestedMeta,
};

pub fn derive(input: TokenStream) -> syn::Result<TokenStream> {
    let item: DeriveInput = syn::parse2(input)?;
    let attrs = Attrs::parse(&item.attrs)?;
    let name = attrs.name.clone().unwrap_or_else(|| item.ident.to_string());
    let generics: Vec<String> = item.generics.type_params().map(|p| p.ident.to_string()).collect();
    let ctx = Ctx;
    let head = if generics.is_empty() { name.clone() } else { format!("{name}<{}>", generics.join(", ")) };

    let mut out = doc_lines(&item.attrs);
    match &item.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(fields) if !attrs.transparent => {
                let mut parents = Vec::new();
                let mut lines = String::new();
                for f in &fields.named {
                    let fa = Attrs::parse(&f.attrs)?;
                    if fa.skip {
                        continue;
                    }
                    if fa.flatten {
                        parents.push(ctx.ty(&f.ty));
                        continue;
                    }
                    let ident = f.ident.as_ref().unwrap().to_string();
                    let key = fa.name.clone().unwrap_or_else(|| rename_field(ident.trim_start_matches("r#"), attrs.rename_all.as_deref()));
                    let (ty, optional) = match &fa.ty {
                        Some(t) => (t.clone(), fa.optional),
                        None => match option_inner(&f.ty) {
                            Some(inner) => (ctx.ty(inner), true),
                            None => (ctx.ty(&f.ty), fa.optional),
                        },
                    };
                    let doc = doc_text(&f.attrs);
                    let doc = if doc.is_empty() { String::new() } else { format!(" {doc}") };
                    lines.push_str(&format!("---@field {}{} {ty}{doc}\n", field_key(&key), if optional { "?" } else { "" }));
                }
                let parents = if parents.is_empty() { String::new() } else { format!(": {}", parents.join(", ")) };
                out.push_str(&format!("---@class {head}{parents}\n{lines}"));
            }
            Fields::Named(fields) => {
                let f = fields.named.first().ok_or_else(|| syn::Error::new_spanned(&item.ident, "transparent needs a field"))?;
                out.push_str(&format!("---@alias {head} {}\n", ctx.ty(&f.ty)));
            }
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                out.push_str(&format!("---@alias {head} {}\n", ctx.ty(&fields.unnamed[0].ty)));
            }
            Fields::Unnamed(fields) => {
                let tys: Vec<_> = fields.unnamed.iter().map(|f| ctx.ty(&f.ty)).collect();
                out.push_str(&format!("---@alias {head} [{}]\n", tys.join(", ")));
            }
            Fields::Unit => out.push_str(&format!("---@alias {head} nil\n")),
        },
        Data::Enum(e) => {
            out.push_str(&format!("---@alias {head}\n"));
            for v in &e.variants {
                let va = Attrs::parse(&v.attrs)?;
                if va.skip {
                    continue;
                }
                let tag = va.name.clone().unwrap_or_else(|| rename_variant(&v.ident.to_string(), attrs.rename_all.as_deref()));
                let tag_lit = format!("{tag:?}");
                let payload = match &v.fields {
                    Fields::Unit => None,
                    Fields::Unnamed(f) if f.unnamed.len() == 1 => Some(Payload::Type(ctx.ty(&f.unnamed[0].ty))),
                    Fields::Unnamed(f) => {
                        Some(Payload::Type(format!("[{}]", f.unnamed.iter().map(|f| ctx.ty(&f.ty)).collect::<Vec<_>>().join(", "))))
                    }
                    Fields::Named(f) => {
                        let mut items = Vec::new();
                        for field in &f.named {
                            let fa = Attrs::parse(&field.attrs)?;
                            if fa.skip {
                                continue;
                            }
                            let ident = field.ident.as_ref().unwrap().to_string();
                            let key = fa.name.clone().unwrap_or_else(|| rename_field(ident.trim_start_matches("r#"), va.rename_all.as_deref()));
                            let (ty, optional) = match option_inner(&field.ty) {
                                Some(inner) => (ctx.ty(inner), true),
                                None => (ctx.ty(&field.ty), fa.optional),
                            };
                            items.push(format!("{}{}: {ty}", field_key(&key), if optional { "?" } else { "" }));
                        }
                        Some(Payload::Fields(items))
                    }
                };
                let alt = match (&attrs.tag, &attrs.content, attrs.untagged, payload) {
                    (_, _, true, None) => "nil".to_string(),
                    (_, _, true, Some(Payload::Type(t))) => t,
                    (_, _, true, Some(Payload::Fields(items))) => format!("{{ {} }}", items.join(", ")),
                    (Some(tag), Some(content), _, Some(p)) => {
                        format!("{{ {}: {tag_lit}, {}: {} }}", field_key(tag), field_key(content), p.inline())
                    }
                    (Some(tag), _, _, None) => format!("{{ {}: {tag_lit} }}", field_key(tag)),
                    (Some(tag), None, _, Some(Payload::Fields(mut items))) => {
                        items.insert(0, format!("{}: {tag_lit}", field_key(tag)));
                        format!("{{ {} }}", items.join(", "))
                    }
                    // serde puts the tag next to a newtype's own fields; LuaLS cannot merge
                    // a named type into a literal, so the tag alone is the best hint.
                    (Some(tag), None, _, Some(Payload::Type(_))) => format!("{{ {}: {tag_lit} }}", field_key(tag)),
                    (None, _, _, None) => tag_lit,
                    (None, _, _, Some(p)) => format!("{{ {}: {} }}", field_key(&tag), p.inline()),
                };
                let doc = doc_text(&v.attrs);
                let doc = if doc.is_empty() { String::new() } else { format!(" # {doc}") };
                out.push_str(&format!("---| {alt}{doc}\n"));
            }
        }
        Data::Union(u) => return Err(syn::Error::new_spanned(u.union_token, "LuaType does not support unions")),
    }

    let lit = LitStr::new(&out, proc_macro2::Span::call_site());
    let name_lit = LitStr::new(&name, proc_macro2::Span::call_site());
    Ok(quote! {
        ::ngs_olezhek::__private::inventory::submit! {
            ::ngs_olezhek::__private::LuaClass { name: #name_lit, source: #lit }
        }
    })
}

enum Payload {
    Type(String),
    Fields(Vec<String>),
}

impl Payload {
    fn inline(self) -> String {
        match self {
            Payload::Type(t) => t,
            Payload::Fields(items) => format!("{{ {} }}", items.join(", ")),
        }
    }
}

struct Ctx;

impl Ctx {
    fn ty(&self, ty: &Type) -> String {
        match ty {
            Type::Reference(r) => self.ty(&r.elem),
            Type::Paren(p) => self.ty(&p.elem),
            Type::Group(g) => self.ty(&g.elem),
            Type::Slice(s) => self.array(&s.elem),
            Type::Array(a) => self.array(&a.elem),
            Type::Tuple(t) if t.elems.is_empty() => "nil".into(),
            Type::Tuple(t) => format!("[{}]", t.elems.iter().map(|e| self.ty(e)).collect::<Vec<_>>().join(", ")),
            Type::Path(p) if p.qself.is_none() => {
                let seg = p.path.segments.last().unwrap();
                let name = seg.ident.to_string();
                let args: Vec<&Type> = match &seg.arguments {
                    PathArguments::AngleBracketed(a) => {
                        a.args.iter().filter_map(|g| if let GenericArgument::Type(t) = g { Some(t) } else { None }).collect()
                    }
                    _ => Vec::new(),
                };
                match name.as_str() {
                    "bool" => "boolean".into(),
                    "Self" => "any".into(),
                    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
                    | "NonZeroU8" | "NonZeroU16" | "NonZeroU32" | "NonZeroU64" | "NonZeroUsize" | "NonZeroI32"
                    | "NonZeroI64" => "integer".into(),
                    "f32" | "f64" => "number".into(),
                    "String" | "str" | "char" | "PathBuf" | "Path" | "OsString" | "Uuid" | "DateTime" | "NaiveDate"
                    | "NaiveDateTime" | "NaiveTime" | "OffsetDateTime" | "PrimitiveDateTime" | "IpAddr" | "Ipv4Addr" | "Ipv6Addr" | "SocketAddr" | "Url" | "Decimal" => "string".into(),
                    "Value" if args.is_empty() => "any".into(),
                    "Map" if args.len() == 2 => format!("table<string, {}>", self.ty(args[1])),
                    "Option" if args.len() == 1 => {
                        let inner = self.ty(args[0]);
                        if inner.ends_with('?') { inner } else { format!("{}?", wrap(&inner)) }
                    }
                    "Vec" | "VecDeque" | "LinkedList" | "HashSet" | "BTreeSet" | "IndexSet" | "BinaryHeap" | "SmallVec"
                        if args.len() == 1 =>
                    {
                        self.array(args[0])
                    }
                    // JSON object keys are strings whatever the Rust key type was.
                    "HashMap" | "BTreeMap" | "IndexMap" if args.len() >= 2 => format!("table<string, {}>", self.ty(args[1])),
                    "Box" | "Rc" | "Arc" | "Cow" | "RefCell" | "Cell" | "Mutex" | "RwLock" | "Json" | "Reverse" | "Wrapping"
                        if !args.is_empty() =>
                    {
                        self.ty(args[args.len() - 1])
                    }
                    "Duration" => "{ secs: integer, nanos: integer }".into(),
                    _ if args.is_empty() => name,
                    _ => format!("{name}<{}>", args.iter().map(|a| self.ty(a)).collect::<Vec<_>>().join(", ")),
                }
            }
            _ => "any".into(),
        }
    }

    fn array(&self, elem: &Type) -> String {
        format!("{}[]", wrap(&self.ty(elem)))
    }
}

fn wrap(t: &str) -> String {
    if t.contains(['|', '?', ' ']) && !t.starts_with('{') && !t.starts_with("table<") { format!("({t})") } else { t.to_string() }
}

fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(a) = &seg.arguments else { return None };
    match a.args.first()? {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    }
}

/// Keys that are not Lua identifiers need the `["..."]` form.
fn field_key(key: &str) -> String {
    let ident = key.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ident { key.to_string() } else { format!("[{key:?}]") }
}

fn doc_text(attrs: &[Attribute]) -> String {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            Meta::NameValue(nv) => match &nv.value {
                Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => Some(s.value().trim().to_string()),
                _ => None,
            },
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn doc_lines(attrs: &[Attribute]) -> String {
    let text = doc_text(attrs);
    if text.is_empty() { String::new() } else { format!("--- {text}\n") }
}

fn words_snake(s: &str) -> Vec<String> {
    s.split('_').filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

fn words_pascal(s: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for c in s.chars() {
        if c.is_uppercase() || words.is_empty() {
            words.push(String::new());
        }
        words.last_mut().unwrap().extend(c.to_lowercase());
    }
    words
}

fn join_case(words: &[String], rule: &str) -> Option<String> {
    let cap = |w: &String| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
    };
    Some(match rule {
        "lowercase" => words.concat(),
        "UPPERCASE" => words.concat().to_uppercase(),
        "PascalCase" => words.iter().map(cap).collect(),
        "camelCase" => words.iter().enumerate().map(|(i, w)| if i == 0 { w.clone() } else { cap(w) }).collect(),
        "snake_case" => words.join("_"),
        "SCREAMING_SNAKE_CASE" => words.join("_").to_uppercase(),
        "kebab-case" => words.join("-"),
        "SCREAMING-KEBAB-CASE" => words.join("-").to_uppercase(),
        _ => return None,
    })
}

fn rename_field(ident: &str, rule: Option<&str>) -> String {
    rule.and_then(|r| join_case(&words_snake(ident), r)).unwrap_or_else(|| ident.to_string())
}

fn rename_variant(ident: &str, rule: Option<&str>) -> String {
    rule.and_then(|r| join_case(&words_pascal(ident), r)).unwrap_or_else(|| ident.to_string())
}

/// The subset of `#[serde(...)]` that changes the JSON shape, plus `#[lua(...)]` overrides.
#[derive(Default)]
struct Attrs {
    name: Option<String>,
    rename_all: Option<String>,
    skip: bool,
    flatten: bool,
    optional: bool,
    transparent: bool,
    untagged: bool,
    tag: Option<String>,
    content: Option<String>,
    ty: Option<String>,
}

impl Attrs {
    fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut out = Attrs::default();
        for attr in attrs {
            let lua = attr.path().is_ident("lua");
            if !lua && !attr.path().is_ident("serde") {
                continue;
            }
            if !matches!(attr.meta, Meta::List(_)) {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                let key = meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                match key.as_str() {
                    "rename" | "name" if lua || key == "rename" => {
                        if let Some(v) = serialize_value(&meta)? {
                            out.name = Some(v);
                        }
                    }
                    "rename_all" if !lua => {
                        if let Some(v) = serialize_value(&meta)? {
                            out.rename_all = Some(v);
                        }
                    }
                    "skip" | "skip_serializing" => out.skip = true,
                    "skip_serializing_if" if !lua => {
                        skip_value(&meta)?;
                        out.optional = true;
                    }
                    "flatten" if !lua => out.flatten = true,
                    "transparent" if !lua => out.transparent = true,
                    "untagged" if !lua => out.untagged = true,
                    "tag" if !lua => out.tag = serialize_value(&meta)?,
                    "content" if !lua => out.content = serialize_value(&meta)?,
                    "type" if lua => out.ty = serialize_value(&meta)?,
                    "optional" if lua => out.optional = true,
                    _ if lua => {
                        return Err(meta.error("expected lua(type = \"...\"), lua(name = \"...\"), lua(optional) or lua(skip)"));
                    }
                    _ => skip_value(&meta)?,
                }
                Ok(())
            })?;
        }
        Ok(out)
    }
}

/// `x = "v"` or `x(serialize = "v", deserialize = "w")`: the serialize side, since the types
/// describe what the server sends.
fn serialize_value(meta: &ParseNestedMeta) -> syn::Result<Option<String>> {
    if meta.input.peek(Token![=]) {
        return Ok(Some(meta.value()?.parse::<LitStr>()?.value()));
    }
    let mut found = None;
    if meta.input.peek(syn::token::Paren) {
        meta.parse_nested_meta(|inner| {
            if inner.path.is_ident("serialize") {
                found = Some(inner.value()?.parse::<LitStr>()?.value());
            } else {
                skip_value(&inner)?;
            }
            Ok(())
        })?;
    }
    Ok(found)
}

/// Serde attributes that do not change the JSON shape (`with`, `bound(...)`, `default = "f"`).
fn skip_value(meta: &ParseNestedMeta) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        meta.value()?.parse::<Expr>()?;
    } else if meta.input.peek(syn::token::Paren) {
        meta.parse_nested_meta(|inner| skip_value(&inner))?;
    }
    Ok(())
}
