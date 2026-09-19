extern crate fxhash;
extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro::{Delimiter, TokenStream, TokenTree};

mod builder;
use builder::Builder;

type Key<'a> = syn::Lit;
type Value<'a> = &'a str;

// Read token groups rather than depending on rustc's TokenStream display whitespace.
fn map_input(input: TokenStream) -> TokenStream {
    let body = group(input, Delimiter::Brace);
    let invocation = group(body, Delimiter::Parenthesis);
    // The static_map! invocation starts with `@zero`.
    invocation.into_iter().skip(2).collect()
}

fn group(input: TokenStream, delimiter: Delimiter) -> TokenStream {
    input.into_iter().find_map(|token| match token {
        TokenTree::Group(group) if group.delimiter() == delimiter => Some(group.stream()),
        _ => None,
    }).expect("invalid static_map! wrapper")
}

#[proc_macro_derive(StaticMapMacro)]
pub fn static_map_macro(input: TokenStream) -> TokenStream {
    let s = map_input(input).to_string();
    let result = build_static_map(&s);

    let wrapper = quote! {
        macro_rules! __static_map__construct_map {
            () => ( #result )
        }
    };

    wrapper.parse().unwrap()
    // panic!("{}", wrapper.to_string());
}

fn build_static_map(input: &str) -> quote::Tokens {
    // TODO: Splitting by @ and ? like this is not hygenic.
    //  This could fail if a value is "This @ is a ? value";
    let mut tt = input.split('@');

    // Extract the defualt value, which should be first token listed.
    // `.split()` always returns at least an empty string for the first iteration.
    let default_value = tt.next().unwrap();
    
    // "@ Key ? Value" pairs follow
    let count = input.chars().filter(|&c| c == '@').count();
    let mut builder = Builder::with_capacity(count);

    // Determine the default key from the first key-value piar
    let mut pair = tt
        .next()
        .expect("staticmap! requires at least one key/value pair")
        .split('?');

    let default_key = syn::parse::lit(pair.next().unwrap()).expect("failed to parse key type");
    let value = pair.next().unwrap();
    builder.insert(default_key.clone(), value);

    for pair in tt {
        let mut pair = pair.split('?');
        
        // Each pair is gauranteed to have a `?`.  So unwrapping is safe.
        let key = syn::parse::lit(pair.next().unwrap()).expect("failed to parse key type");
        let value = pair.next().unwrap();
        builder.insert(key, value);
    }

    builder.build(lit_default(&default_key), default_value)
}

fn lit_default(lit: &syn::Lit) -> syn::Lit {
    use syn::Lit::*;
    use syn::Lit;

    match *lit {
        Str(_, _) => Lit::from(""),
        Byte(_) => Lit::from(0u8),
        Char(_) => Lit::from(0 as char),
        Int(_, ty) => {
            use syn::IntTy::*;
            use syn::IntTy;
            match ty {
                Isize => Lit::from(0isize),
                I8 => Lit::from(0i8),
                I16 => Lit::from(0i16),
                I32 => Lit::from(0i32),
                I64 => Lit::from(0i64),
                Usize => Lit::from(0usize),
                U8 => Lit::from(0u8),
                U16 => Lit::from(0u16),
                U32 => Lit::from(0u32),
                U64 => Lit::from(0u64),
                Unsuffixed => Lit::Int(0, IntTy::Unsuffixed),
            }
        },
        ref lit => panic!("staticmap! unsupported key type `{:?}`", lit),
    }
}
