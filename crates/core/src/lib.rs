//! Shared data model, signature parsing, and code generation for rust-playgraph.
#![allow(clippy::result_large_err)]

pub mod api;
pub mod codegen;
pub mod highlight;
pub mod model;
pub mod parse;
pub mod unify;

pub use api::{ExecuteResponse, GenerateResponse};
pub use codegen::{CodegenError, generate};
pub use highlight::{TokenKind, tokenize};
pub use model::{Cache, Edge, Eval, GraphNode, OutputDef, ParamDef, Playground, StageDef};
pub use parse::{
    ParseError, Port, RefKind, StageSignature, is_valid_ident, normalize_type, parse_signature,
    parse_stage, ref_kind_of, substitute_type, type_to_string,
};
pub use unify::{UnifiedArgs, propagate_types, unify_ports};
