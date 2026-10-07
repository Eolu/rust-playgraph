//! Lightweight signature extraction from a stage's Rust `fn` source.

use syn::{FnArg, ItemFn, Pat, ReturnType, Type};
use thiserror::Error;

/// How an input is passed to the stage function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Owned,
    Borrowed,
    BorrowedMut,
}

/// A named, typed port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    pub name: String,
    pub type_name: String,
    pub ref_kind: RefKind,
}

/// The parsed shape of a stage function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageSignature {
    pub fn_name: String,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
}

#[derive(Debug, Clone, Error)]
pub enum ParseError {
    #[error("failed to parse function: {0}")]
    Syn(String),
    #[error("unsupported input `{0}`; only simple identifiers are supported")]
    UnsupportedInput(String),
}

/// Parse a complete function (signature + body) and extract its ports.
pub fn parse_stage(code: &str) -> Result<StageSignature, ParseError> {
    let item: ItemFn = syn::parse_str(code).map_err(|error| ParseError::Syn(error.to_string()))?;
    signature_from_item(&item)
}

/// Parse just a function header, e.g. `fn add(a: i32, b: i32) -> i32`.
pub fn parse_signature(signature: &str) -> Result<StageSignature, ParseError> {
    let source = format!("{}\n{{}}", signature.trim());
    let item: ItemFn =
        syn::parse_str(&source).map_err(|error| ParseError::Syn(error.to_string()))?;
    signature_from_item(&item)
}

fn signature_from_item(item: &ItemFn) -> Result<StageSignature, ParseError> {
    let fn_name = item.sig.ident.to_string();
    let mut inputs = Vec::new();
    for arg in &item.sig.inputs {
        let FnArg::Typed(pat_type) = arg else {
            return Err(ParseError::UnsupportedInput("self".to_string()));
        };
        let Pat::Ident(pat_ident) = &*pat_type.pat else {
            return Err(ParseError::UnsupportedInput(
                quote::quote!(#pat_type.pat).to_string(),
            ));
        };
        let raw = pat_ident.ident.to_string();
        let name = raw.strip_prefix('_').unwrap_or(&raw).to_string();
        let (ref_kind, type_name) = match &*pat_type.ty {
            Type::Reference(reference) => {
                let kind = if reference.mutability.is_some() {
                    RefKind::BorrowedMut
                } else {
                    RefKind::Borrowed
                };
                (kind, type_to_string(&reference.elem))
            }
            other => (RefKind::Owned, type_to_string(other)),
        };
        inputs.push(Port {
            name,
            type_name,
            ref_kind,
        });
    }

    let output_type = match &item.sig.output {
        ReturnType::Type(_, ty) => type_to_string(ty),
        ReturnType::Default => "()".to_string(),
    };
    let outputs = vec![Port {
        name: "out".to_string(),
        type_name: output_type,
        ref_kind: RefKind::Owned,
    }];

    Ok(StageSignature {
        fn_name,
        inputs,
        outputs,
    })
}

/// Render a type as a compact, whitespace-free string for display and
/// comparison.
pub fn type_to_string(ty: &Type) -> String {
    quote::quote!(#ty).to_string().replace(' ', "")
}

/// Normalize a user-entered type string (e.g. `Vec< i32 >` -> `Vec<i32>`).
/// Falls back to the trimmed input if it is not a parseable type.
pub fn normalize_type(text: &str) -> String {
    match syn::parse_str::<Type>(text.trim()) {
        Ok(ty) => type_to_string(&ty),
        Err(_) => text.trim().to_string(),
    }
}

/// Classify a user-entered input type.
pub fn ref_kind_of(text: &str) -> RefKind {
    match syn::parse_str::<Type>(text.trim()) {
        Ok(Type::Reference(reference)) if reference.mutability.is_some() => RefKind::BorrowedMut,
        Ok(Type::Reference(_)) => RefKind::Borrowed,
        _ => RefKind::Owned,
    }
}

/// Whether `name` is a usable function/type identifier.
pub fn is_valid_ident(name: &str) -> bool {
    syn::parse_str::<syn::Ident>(name.trim()).is_ok()
}

/// Replace type parameters (e.g. `T`) in `ty` with the mapped concrete types.
pub fn substitute_type(ty: &Type, map: &std::collections::HashMap<String, Type>) -> Type {
    match ty {
        Type::Path(path) => {
            // A bare parameter, e.g. `T`.
            if path.qself.is_none()
                && path.path.segments.len() == 1
                && path.path.segments[0].arguments.is_none()
                && let Some(replacement) = map.get(&path.path.segments[0].ident.to_string())
            {
                return replacement.clone();
            }
            let mut path = path.clone();
            for segment in path.path.segments.iter_mut() {
                if let syn::PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
                    for argument in arguments.args.iter_mut() {
                        if let syn::GenericArgument::Type(inner) = argument {
                            *inner = substitute_type(inner, map);
                        }
                    }
                }
            }
            Type::Path(path)
        }
        Type::Reference(reference) => {
            let mut reference = reference.clone();
            reference.elem = Box::new(substitute_type(&reference.elem, map));
            Type::Reference(reference)
        }
        Type::Tuple(tuple) => {
            let mut tuple = tuple.clone();
            for element in tuple.elems.iter_mut() {
                *element = substitute_type(element, map);
            }
            Type::Tuple(tuple)
        }
        Type::Slice(slice) => {
            let mut slice = slice.clone();
            slice.elem = Box::new(substitute_type(&slice.elem, map));
            Type::Slice(slice)
        }
        Type::Array(array) => {
            let mut array = array.clone();
            array.elem = Box::new(substitute_type(&array.elem, map));
            Type::Array(array)
        }
        Type::Paren(paren) => {
            let mut paren = paren.clone();
            paren.elem = Box::new(substitute_type(&paren.elem, map));
            Type::Paren(paren)
        }
        other => other.clone(),
    }
}
