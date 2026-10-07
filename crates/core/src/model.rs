//! The serializable data model shared by the frontend and the backend.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use syn::Type;

use crate::parse::{
    Port, RefKind, StageSignature, normalize_type, ref_kind_of, substitute_type, type_to_string,
};

/// Evaluation strategy for a stage, mirroring `directed::EvalStrategy`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Eval {
    /// Evaluated whenever it is needed and as a graph entry point.
    #[default]
    Urgent,
    /// Only evaluated when an urgent descendant needs it.
    Lazy,
}

/// Caching strategy for a stage, mirroring `directed::CachePolicy`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Cache {
    /// Opaque: re-evaluate every time inputs change (or always).
    #[default]
    None,
    /// Reuse the previous outputs when the inputs are unchanged.
    Last,
    /// Memoize every distinct combination of inputs.
    All,
}

/// A named, typed function parameter.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ParamDef {
    pub name: String,
    pub type_name: String,
}

/// A named output of a stage.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct OutputDef {
    pub name: String,
    pub type_name: String,
}

/// A reusable stage definition: a function assembled from structured fields,
/// plus metadata.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct StageDef {
    /// The function/type name.
    pub name: String,
    /// Optional type parameters, without angle brackets, e.g. `T` or `T, U`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub generics: String,
    /// Optional `where` predicates, without the `where` keyword, e.g. `T: Clone`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub where_clause: String,
    /// Emit `async fn`.
    #[serde(default)]
    pub is_async: bool,
    /// Input parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<ParamDef>,
    /// Named outputs. When empty the stage has a single `out: ()` output.
    /// Otherwise the function returns one value (or a tuple) in this order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<OutputDef>,
    /// The statements inside the function body (no surrounding braces).
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub eval: Eval,
    #[serde(default)]
    pub cache: Cache,
    /// Optional state type, e.g. `u32` or `(u8, u8)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_type: Option<String>,
}

impl StageDef {
    /// The `<...>` part of the header, or empty. Accepts `generics` with or
    /// without the angle brackets.
    pub fn generics_clause(&self) -> String {
        let generics = self.generics.trim();
        if generics.is_empty() {
            String::new()
        } else if generics.starts_with('<') {
            generics.to_string()
        } else {
            format!("<{generics}>")
        }
    }

    /// The ` where ...` part of the header, or empty. Accepts `where_clause`
    /// with or without the leading `where` keyword.
    pub fn where_clause_fragment(&self) -> String {
        let clause = self.where_clause.trim();
        if clause.is_empty() {
            String::new()
        } else {
            let clause = clause
                .strip_prefix("where")
                .map(str::trim)
                .unwrap_or(clause);
            format!(" where {clause}")
        }
    }

    /// Reconstruct the function header, e.g. `fn Add(a: i32, b: i32) -> i32`.
    pub fn header(&self) -> String {
        let async_kw = if self.is_async { "async " } else { "" };
        let params = self
            .inputs
            .iter()
            .map(|param| format!("{}: {}", param.name.trim(), param.type_name.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        let ret = match self.outputs.len() {
            0 => String::new(),
            1 => format!(" -> {}", self.outputs[0].type_name.trim()),
            _ => format!(
                " -> ({})",
                self.outputs
                    .iter()
                    .map(|output| output.type_name.trim().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        format!(
            "{async_kw}fn {}{}({params}){ret}{}",
            self.name.trim(),
            self.generics_clause(),
            self.where_clause_fragment()
        )
    }

    /// Reassemble the full Rust function source.
    pub fn source(&self) -> String {
        format!("{} {{\n{}\n}}", self.header(), self.body.trim_end())
    }

    /// The type parameter names declared in `generics`, if any.
    pub fn type_params(&self) -> Vec<String> {
        let generics = self.generics_clause();
        if generics.is_empty() {
            return Vec::new();
        }
        let source = format!("fn __stage__{generics}() {{}}");
        syn::parse_str::<syn::ItemFn>(&source)
            .map(|item| {
                item.sig
                    .generics
                    .type_params()
                    .map(|param| param.ident.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The ports for a node of this stage, substituting concrete type arguments
    /// for the stage's type parameters.
    pub fn node_signature(&self, type_args: &[String]) -> StageSignature {
        let base = self.signature_info();
        let params = self.type_params();
        if params.is_empty() || type_args.is_empty() {
            return base;
        }
        let mut map: HashMap<String, Type> = HashMap::new();
        for (param, argument) in params.iter().zip(type_args) {
            if let Ok(ty) = syn::parse_str::<Type>(argument) {
                map.insert(param.clone(), ty);
            }
        }
        let substitute = |type_name: &str| -> String {
            match syn::parse_str::<Type>(type_name) {
                Ok(ty) => type_to_string(&substitute_type(&ty, &map)),
                Err(_) => type_name.to_string(),
            }
        };
        StageSignature {
            fn_name: base.fn_name,
            inputs: base
                .inputs
                .into_iter()
                .map(|port| Port {
                    type_name: substitute(&port.type_name),
                    ..port
                })
                .collect(),
            outputs: base
                .outputs
                .into_iter()
                .map(|port| Port {
                    type_name: substitute(&port.type_name),
                    ..port
                })
                .collect(),
        }
    }

    /// Build the port description used for display, validation, and codegen.
    pub fn signature_info(&self) -> StageSignature {
        let mut outputs: Vec<Port> = self
            .outputs
            .iter()
            .map(|output| Port {
                name: output.name.clone(),
                type_name: normalize_type(&output.type_name),
                ref_kind: RefKind::Owned,
            })
            .collect();
        // The `directed` macro always produces at least one output; with none
        // declared it is a single `out: ()`.
        if outputs.is_empty() {
            outputs.push(Port {
                name: "out".to_string(),
                type_name: "()".to_string(),
                ref_kind: RefKind::Owned,
            });
        }
        StageSignature {
            fn_name: self.name.clone(),
            inputs: self
                .inputs
                .iter()
                .map(|param| Port {
                    name: param.name.clone(),
                    type_name: normalize_type(&param.type_name),
                    ref_kind: ref_kind_of(&param.type_name),
                })
                .collect(),
            outputs,
        }
    }
}

/// A node instance placed on the canvas.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GraphNode {
    pub id: String,
    /// The stage name this node instantiates.
    pub stage: String,
    /// Concrete type arguments for a generic stage, positionally matched to
    /// its type parameters. Missing entries are inferred (`_`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_args: Vec<String>,
    pub x: f64,
    pub y: f64,
}

/// A directed connection from one node output to another node input.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Edge {
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
}

/// The whole document: stage definitions plus the composed graph.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Playground {
    #[serde(default)]
    pub stages: Vec<StageDef>,
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    /// Free-form Rust emitted at the outermost scope of the generated program,
    /// before the stage definitions. Use it for imports, helpers, types, etc.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prelude: String,
}

impl Playground {
    /// Find a stage by name.
    pub fn stage(&self, name: &str) -> Option<&StageDef> {
        self.stages.iter().find(|stage| stage.name == name)
    }

    pub fn node(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
}
