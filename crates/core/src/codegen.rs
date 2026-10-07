//! Turn a [`Playground`] into a single, runnable Rust program that uses the
//! real `directed` crate.

use std::collections::{HashMap, HashSet};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ItemFn, Type};
use thiserror::Error;

use crate::model::{Cache, Edge, Eval, GraphNode, Playground, StageDef};
use crate::parse::{ParseError, StageSignature};
use crate::unify::unify_ports;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("duplicate stage function name `{0}`")]
    DuplicateStage(String),
    #[error("`{0}` is not a valid Rust identifier")]
    InvalidName(String),
    #[error("the stage name `main` is reserved")]
    ReservedName,
    #[error("stage `{stage}` could not be parsed: {source}")]
    Parse {
        stage: String,
        #[source]
        source: ParseError,
    },
    #[error("stage `{stage}` has an invalid state type: {message}")]
    InvalidStateType { stage: String, message: String },
    #[error("stage `{stage}` has an invalid output type `{ty}`")]
    InvalidOutputType { stage: String, ty: String },
    #[error("node `{0}` uses a stage that is not defined")]
    UnknownStage(String),
    #[error("an edge references the unknown node `{0}`")]
    UnknownNode(String),
    #[error("node `{node}` has no {kind} port named `{port}`")]
    UnknownPort {
        node: String,
        kind: &'static str,
        port: String,
    },
    #[error(
        "cannot connect `{from_node}.{from_port}` ({from_ty}) to `{to_node}.{to_port}` ({to_ty}): types differ"
    )]
    TypeMismatch {
        from_node: String,
        from_port: String,
        from_ty: String,
        to_node: String,
        to_port: String,
        to_ty: String,
    },
    #[error("the graph contains a cycle involving node `{0}`")]
    Cycle(String),
    #[error("internal codegen error: {0}")]
    Internal(String),
}

/// Generate a complete `main.rs` for the given playground.
pub fn generate(playground: &Playground) -> Result<String, CodegenError> {
    // Parse each stage signature and index it by derived function name.
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for (index, stage) in playground.stages.iter().enumerate() {
        if stage.name == "main" {
            return Err(CodegenError::ReservedName);
        }
        if !crate::parse::is_valid_ident(&stage.name) {
            return Err(CodegenError::InvalidName(stage.name.clone()));
        }
        if by_name.insert(stage.name.clone(), index).is_some() {
            return Err(CodegenError::DuplicateStage(stage.name.clone()));
        }
    }

    let node_index: HashMap<&str, usize> = playground
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect();
    for node in &playground.nodes {
        if !by_name.contains_key(&node.stage) {
            return Err(CodegenError::UnknownStage(node.id.clone()));
        }
    }
    // Per-node signatures with concrete type arguments substituted.
    let node_signatures: Vec<StageSignature> = playground
        .nodes
        .iter()
        .map(|node| playground.stages[by_name[&node.stage]].node_signature(&node.type_args))
        .collect();

    validate_edges(playground, &node_index, &node_signatures)?;
    if let Some(node) = detect_cycle(&playground.nodes, &playground.edges) {
        return Err(CodegenError::Cycle(node));
    }

    // Stage function items: header + body, with metadata as `#[stage(...)]`.
    let mut stage_items = Vec::new();
    for stage in &playground.stages {
        stage_items.push(stage_item(stage)?);
    }

    let node_vars: Vec<Ident> = (0..playground.nodes.len())
        .map(|index| format_ident!("node_{index}"))
        .collect();
    let stage_types: Vec<Ident> = playground
        .nodes
        .iter()
        .map(|node| Ident::new(&node.stage, proc_macro2::Span::call_site()))
        .collect();
    let node_turbofish: Vec<TokenStream> = playground
        .nodes
        .iter()
        .map(|node| turbofish(&playground.stages[by_name[&node.stage]], &node.type_args))
        .collect();

    let connections: Vec<TokenStream> = playground
        .edges
        .iter()
        .map(|edge| {
            let from = &node_vars[node_index[edge.from_node.as_str()]];
            let to = &node_vars[node_index[edge.to_node.as_str()]];
            let from_port = Ident::new(&edge.from_port, proc_macro2::Span::call_site());
            let to_port = Ident::new(&edge.to_port, proc_macro2::Span::call_site());
            quote! { #from: #from_port => #to: #to_port }
        })
        .collect();

    // Targets: urgent nodes plus sinks (nodes with no outgoing edge). These
    // drive execution; the stages themselves are responsible for any output
    // (e.g. via the library's `Print` / `Dbg` stages).
    let has_outgoing: HashSet<&str> = playground
        .edges
        .iter()
        .map(|edge| edge.from_node.as_str())
        .collect();
    let targets: Vec<usize> = playground
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            let is_sink = !has_outgoing.contains(node.id.as_str());
            let is_urgent = playground
                .stage(&node.stage)
                .map(|stage| stage.eval == Eval::Urgent)
                .unwrap_or(false);
            is_sink || is_urgent
        })
        .map(|(index, _)| index)
        .collect();

    let target_vars: Vec<&Ident> = targets.iter().map(|&index| &node_vars[index]).collect();

    let main: ItemFn = syn::parse2(quote! {
        fn main() {
            let mut registry = directed::Registry::new();
            #(let #node_vars = registry.register::<#stage_types #node_turbofish>();)*
            let graph = directed::graph! {
                nodes: [#(#node_vars),*],
                connections: { #(#connections),* }
            }
            .unwrap();
            let targets: Vec<directed::NodeId> = vec![#(#target_vars.id()),*];
            directed::block_on(graph.execute_async(&registry, &targets)).unwrap();
        }
    })
    .map_err(|error| CodegenError::Internal(error.to_string()))?;

    let body: syn::File = syn::parse2(quote! {
        #(#stage_items)*
        #main
    })
    .map_err(|error| CodegenError::Internal(error.to_string()))?;

    // The header is emitted verbatim so the outer-scope `prelude` keeps the
    // user's own formatting and comments.
    let mut source = String::from(
        "#![allow(unused_imports, unused_variables, dead_code)]\nuse directed::{stage, StageHandle};\n",
    );
    let prelude = playground.prelude.trim();
    if !prelude.is_empty() {
        source.push('\n');
        source.push_str(prelude);
        source.push('\n');
    }
    source.push('\n');
    source.push_str(&prettyplease::unparse(&body));
    Ok(source)
}

fn validate_edges(
    playground: &Playground,
    node_index: &HashMap<&str, usize>,
    node_signatures: &[StageSignature],
) -> Result<(), CodegenError> {
    for edge in &playground.edges {
        let from_index = node_index
            .get(edge.from_node.as_str())
            .copied()
            .ok_or_else(|| CodegenError::UnknownNode(edge.from_node.clone()))?;
        let to_index = node_index
            .get(edge.to_node.as_str())
            .copied()
            .ok_or_else(|| CodegenError::UnknownNode(edge.to_node.clone()))?;
        let from_sig = &node_signatures[from_index];
        let to_sig = &node_signatures[to_index];

        let output = from_sig
            .outputs
            .iter()
            .find(|port| port.name == edge.from_port)
            .ok_or_else(|| CodegenError::UnknownPort {
                node: edge.from_node.clone(),
                kind: "output",
                port: edge.from_port.clone(),
            })?;
        let input = to_sig
            .inputs
            .iter()
            .find(|port| port.name == edge.to_port)
            .ok_or_else(|| CodegenError::UnknownPort {
                node: edge.to_node.clone(),
                kind: "input",
                port: edge.to_port.clone(),
            })?;
        if output.type_name != input.type_name {
            // A mismatch may still be fine when one side is an unresolved type
            // parameter: the generated `Foo::<_>` lets the compiler infer it
            // from the connection. Only reject genuinely incompatible types.
            let from_node = &playground.nodes[from_index];
            let to_node = &playground.nodes[to_index];
            let (Some(from_stage), Some(to_stage)) = (
                playground.stage(&from_node.stage),
                playground.stage(&to_node.stage),
            ) else {
                continue;
            };
            if unify_ports(
                &from_stage.type_params(),
                &from_node.type_args,
                &output.type_name,
                &to_stage.type_params(),
                &to_node.type_args,
                &input.type_name,
            )
            .is_none()
            {
                return Err(CodegenError::TypeMismatch {
                    from_node: edge.from_node.clone(),
                    from_port: edge.from_port.clone(),
                    from_ty: output.type_name.clone(),
                    to_node: edge.to_node.clone(),
                    to_port: edge.to_port.clone(),
                    to_ty: input.type_name.clone(),
                });
            }
        }
    }
    Ok(())
}

/// The turbofish for registering a node, filling unspecified arguments with
/// `_` so the compiler can infer them.
fn turbofish(stage: &StageDef, type_args: &[String]) -> TokenStream {
    let params = stage.type_params();
    if params.is_empty() {
        return quote!();
    }
    let mut args: Vec<Type> = type_args
        .iter()
        .take(params.len())
        .filter_map(|argument| syn::parse_str::<Type>(argument).ok())
        .collect();
    while args.len() < params.len() {
        args.push(syn::parse_quote!(_));
    }
    quote!(::<#(#args),*>)
}

fn detect_cycle(nodes: &[GraphNode], edges: &[Edge]) -> Option<String> {
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        adjacency
            .entry(edge.from_node.as_str())
            .or_default()
            .push(edge.to_node.as_str());
    }

    fn visit<'a>(
        node: &'a str,
        adjacency: &HashMap<&'a str, Vec<&'a str>>,
        state: &mut HashMap<&'a str, u8>,
    ) -> bool {
        match state.get(node) {
            Some(1) => return true,
            Some(2) => return false,
            _ => {}
        }
        state.insert(node, 1);
        if let Some(next) = adjacency.get(node) {
            for &child in next {
                if visit(child, adjacency, state) {
                    return true;
                }
            }
        }
        state.insert(node, 2);
        false
    }

    let mut state = HashMap::new();
    for node in nodes {
        if visit(node.id.as_str(), &adjacency, &mut state) {
            return Some(node.id.clone());
        }
    }
    None
}

fn stage_item(stage: &StageDef) -> Result<ItemFn, CodegenError> {
    let source = stage.source();
    let mut item: ItemFn = syn::parse_str(&source).map_err(|error| CodegenError::Parse {
        stage: stage.name.clone(),
        source: ParseError::Syn(error.to_string()),
    })?;
    // Never leave a user-supplied `#[stage]` in place; metadata owns it.
    item.attrs.retain(|attr| !attr.path().is_ident("stage"));

    let mut args: Vec<TokenStream> = Vec::new();
    if stage.eval == Eval::Lazy {
        args.push(quote!(lazy));
    }
    match stage.cache {
        Cache::Last => args.push(quote!(cache_last)),
        Cache::All => args.push(quote!(cache_all)),
        Cache::None => {}
    }
    if !stage.outputs.is_empty() {
        let mut outs: Vec<TokenStream> = Vec::new();
        for output in &stage.outputs {
            let name: Ident = syn::parse_str(&output.name)
                .map_err(|_| CodegenError::InvalidName(output.name.clone()))?;
            let ty: Type =
                syn::parse_str(&output.type_name).map_err(|_| CodegenError::InvalidOutputType {
                    stage: stage.name.clone(),
                    ty: output.type_name.clone(),
                })?;
            outs.push(quote! { #name: #ty });
        }
        args.push(quote!(out(#(#outs),*)));
    }
    if let Some(state) = &stage.state_type {
        let ty: Type = syn::parse_str(state).map_err(|error| CodegenError::InvalidStateType {
            stage: stage.name.clone(),
            message: error.to_string(),
        })?;
        args.push(quote!(state(#ty)));
    }

    let attribute = if args.is_empty() {
        quote!(#[stage])
    } else {
        quote!(#[stage(#(#args),*)])
    };
    let combined = quote!(#attribute #item);
    syn::parse2(combined).map_err(|error| CodegenError::Internal(error.to_string()))
}
