//! Unify two port types so generic nodes can infer their type arguments when
//! connected to concrete ports.

use std::collections::{HashMap, HashSet};

use syn::{GenericArgument, PathArguments, Type};

use crate::model::{Playground, StageDef};
use crate::parse::type_to_string;

/// The resolved type arguments for each side of a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedArgs {
    pub from: Vec<String>,
    pub to: Vec<String>,
}

/// Attempt to unify an output type with an input type.
///
/// Both type strings are the ports' *instantiated* types (type arguments
/// already substituted), so any remaining type parameter is unresolved. On
/// success, returns the type-argument vectors to use for the `from` and `to`
/// stages.
pub fn unify_ports(
    from_params: &[String],
    from_args: &[String],
    from_type: &str,
    to_params: &[String],
    to_args: &[String],
    to_type: &str,
) -> Option<UnifiedArgs> {
    let from = syn::parse_str::<Type>(from_type.trim()).ok()?;
    let to = syn::parse_str::<Type>(to_type.trim()).ok()?;

    let mut vars: HashSet<String> = HashSet::new();
    vars.extend(from_params.iter().cloned());
    vars.extend(to_params.iter().cloned());

    let mut bindings: HashMap<String, Type> = HashMap::new();
    if !unify(&from, &to, &vars, &mut bindings) {
        return None;
    }

    Some(UnifiedArgs {
        from: resolve_args(from_params, from_args, &vars, &bindings),
        to: resolve_args(to_params, to_args, &vars, &bindings),
    })
}

fn resolve_args(
    params: &[String],
    args: &[String],
    vars: &HashSet<String>,
    bindings: &HashMap<String, Type>,
) -> Vec<String> {
    params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            if let Some(bound) = bindings.get(param)
                && !has_var(bound, vars)
            {
                return type_to_string(bound);
            }
            args.get(index).cloned().unwrap_or_default()
        })
        .collect()
}

/// Propagate type arguments across the whole graph until it stabilises.
///
/// Pinning one node (or connecting to a concrete port) then fills in the
/// connected generic nodes. `library` supplies stages that are not part of the
/// document (e.g. the built-in library). Existing type arguments are never
/// overwritten.
pub fn propagate_types(playground: &mut Playground, library: &[StageDef]) {
    let node_index: HashMap<String, usize> = playground
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.clone(), index))
        .collect();

    // Each pass only ever fills in previously-empty arguments, so this
    // terminates; the cap is just belt-and-braces.
    for _ in 0..playground.edges.len() + playground.nodes.len() + 1 {
        let mut updates: Vec<(usize, Vec<String>)> = Vec::new();
        for edge in &playground.edges {
            let (Some(&from_index), Some(&to_index)) = (
                node_index.get(&edge.from_node),
                node_index.get(&edge.to_node),
            ) else {
                continue;
            };
            let from_node = &playground.nodes[from_index];
            let to_node = &playground.nodes[to_index];
            let from_stage = find_stage(&playground.stages, library, &from_node.stage);
            let to_stage = find_stage(&playground.stages, library, &to_node.stage);
            let (Some(from_stage), Some(to_stage)) = (from_stage, to_stage) else {
                continue;
            };
            let from_sig = from_stage.node_signature(&from_node.type_args);
            let to_sig = to_stage.node_signature(&to_node.type_args);
            let (Some(output), Some(input)) = (
                from_sig
                    .outputs
                    .iter()
                    .find(|port| port.name == edge.from_port),
                to_sig.inputs.iter().find(|port| port.name == edge.to_port),
            ) else {
                continue;
            };
            if output.type_name == input.type_name {
                continue;
            }
            let Some(unified) = unify_ports(
                &from_stage.type_params(),
                &from_node.type_args,
                &output.type_name,
                &to_stage.type_params(),
                &to_node.type_args,
                &input.type_name,
            ) else {
                continue;
            };
            if unified.from != from_node.type_args {
                updates.push((from_index, unified.from));
            }
            if unified.to != to_node.type_args {
                updates.push((to_index, unified.to));
            }
        }
        if updates.is_empty() {
            break;
        }
        for (index, args) in updates {
            playground.nodes[index].type_args = args;
        }
    }
}

fn find_stage<'a>(
    user: &'a [StageDef],
    library: &'a [StageDef],
    name: &str,
) -> Option<&'a StageDef> {
    user.iter()
        .find(|stage| stage.name == name)
        .or_else(|| library.iter().find(|stage| stage.name == name))
}

fn unify(a: &Type, b: &Type, vars: &HashSet<String>, bindings: &mut HashMap<String, Type>) -> bool {
    if let Some(name) = bare_var(a, vars) {
        return bind(&name, b, vars, bindings);
    }
    if let Some(name) = bare_var(b, vars) {
        return bind(&name, a, vars, bindings);
    }
    match (a, b) {
        (Type::Reference(ra), Type::Reference(rb))
            if ra.mutability.is_some() == rb.mutability.is_some() =>
        {
            unify(&ra.elem, &rb.elem, vars, bindings)
        }
        (Type::Path(pa), Type::Path(pb)) if pa.qself.is_none() && pb.qself.is_none() => {
            match (pa.path.segments.last(), pb.path.segments.last()) {
                (Some(sa), Some(sb)) if sa.ident == sb.ident => {
                    let aa = type_args(&sa.arguments);
                    let ba = type_args(&sb.arguments);
                    aa.len() == ba.len()
                        && aa.iter().zip(&ba).all(|(x, y)| unify(x, y, vars, bindings))
                }
                _ => type_to_string(a) == type_to_string(b),
            }
        }
        (Type::Tuple(ta), Type::Tuple(tb)) => {
            ta.elems.len() == tb.elems.len()
                && ta
                    .elems
                    .iter()
                    .zip(&tb.elems)
                    .all(|(x, y)| unify(x, y, vars, bindings))
        }
        (Type::Slice(sa), Type::Slice(sb)) => unify(&sa.elem, &sb.elem, vars, bindings),
        (Type::Array(aa), Type::Array(ab)) => unify(&aa.elem, &ab.elem, vars, bindings),
        (Type::Paren(pa), Type::Paren(pb)) => unify(&pa.elem, &pb.elem, vars, bindings),
        _ => type_to_string(a) == type_to_string(b),
    }
}

/// Bind a variable to a type. Binding to another unresolved variable is a
/// no-op (the compiler will infer it).
fn bind(
    name: &str,
    value: &Type,
    vars: &HashSet<String>,
    bindings: &mut HashMap<String, Type>,
) -> bool {
    let value = resolve(value, vars, bindings);
    if has_var(&value, vars) {
        return true;
    }
    match bindings.get(name).cloned() {
        Some(existing) => unify(&existing, &value, vars, bindings),
        None => {
            bindings.insert(name.to_string(), value);
            true
        }
    }
}

fn resolve(ty: &Type, vars: &HashSet<String>, bindings: &HashMap<String, Type>) -> Type {
    let mut current = ty.clone();
    let mut guard = 0;
    while let Some(name) = bare_var(&current, vars) {
        guard += 1;
        if guard > 32 {
            break;
        }
        match bindings.get(&name) {
            Some(next) => current = next.clone(),
            None => break,
        }
    }
    current
}

fn bare_var(ty: &Type, vars: &HashSet<String>) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = &path.path.segments[0];
    if !matches!(segment.arguments, PathArguments::None) {
        return None;
    }
    let name = segment.ident.to_string();
    vars.contains(&name).then_some(name)
}

fn has_var(ty: &Type, vars: &HashSet<String>) -> bool {
    if bare_var(ty, vars).is_some() {
        return true;
    }
    match ty {
        Type::Reference(reference) => has_var(&reference.elem, vars),
        Type::Slice(slice) => has_var(&slice.elem, vars),
        Type::Array(array) => has_var(&array.elem, vars),
        Type::Paren(paren) => has_var(&paren.elem, vars),
        Type::Tuple(tuple) => tuple.elems.iter().any(|element| has_var(element, vars)),
        Type::Path(path) => path.path.segments.iter().any(|segment| {
            type_args(&segment.arguments)
                .iter()
                .any(|argument| has_var(argument, vars))
        }),
        _ => false,
    }
}

fn type_args(arguments: &PathArguments) -> Vec<&Type> {
    match arguments {
        PathArguments::AngleBracketed(angle) => angle
            .args
            .iter()
            .filter_map(|argument| match argument {
                GenericArgument::Type(ty) => Some(ty),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn infers_bare_param_from_concrete() {
        let result = unify_ports(&params(&["T"]), &[], "T", &params(&[]), &[], "i32").unwrap();
        assert_eq!(result.from, vec!["i32".to_string()]);
    }

    #[test]
    fn infers_inside_container() {
        let result = unify_ports(
            &params(&["T"]),
            &[],
            "Vec<T>",
            &params(&[]),
            &[],
            "Vec<i32>",
        )
        .unwrap();
        assert_eq!(result.from, vec!["i32".to_string()]);
    }

    #[test]
    fn infers_across_two_nodes() {
        let result = unify_ports(&params(&["T"]), &[], "T", &params(&["U"]), &[], "U").unwrap();
        // Two unresolved variables stay unresolved.
        assert_eq!(result.from, vec![String::new()]);
        assert_eq!(result.to, vec![String::new()]);
    }

    #[test]
    fn rejects_incompatible() {
        assert!(unify_ports(&params(&["T"]), &[], "T", &params(&[]), &[], "String").is_some());
        assert!(unify_ports(&params(&[]), &[], "i32", &params(&[]), &[], "String").is_none());
    }

    #[test]
    fn propagates_across_graph() {
        use crate::model::{Edge, GraphNode, OutputDef, ParamDef};

        let generic = |name: &str| StageDef {
            name: name.to_string(),
            generics: "T".to_string(),
            ..StageDef::default()
        };
        let default_value = StageDef {
            outputs: vec![OutputDef {
                name: "out".to_string(),
                type_name: "T".to_string(),
            }],
            ..generic("DefaultValue")
        };
        let one = StageDef {
            outputs: vec![OutputDef {
                name: "out".to_string(),
                type_name: "T".to_string(),
            }],
            ..generic("One")
        };
        let add = StageDef {
            inputs: vec![
                ParamDef {
                    name: "a".to_string(),
                    type_name: "T".to_string(),
                },
                ParamDef {
                    name: "b".to_string(),
                    type_name: "T".to_string(),
                },
            ],
            outputs: vec![OutputDef {
                name: "out".to_string(),
                type_name: "T".to_string(),
            }],
            ..generic("Add")
        };

        let node = |id: &str, stage: &str, type_args: Vec<String>| GraphNode {
            id: id.to_string(),
            stage: stage.to_string(),
            type_args,
            x: 0.0,
            y: 0.0,
        };
        let mut playground = Playground {
            prelude: String::new(),
            stages: Vec::new(),
            nodes: vec![
                node("dv", "DefaultValue", vec!["i32".to_string()]),
                node("one", "One", Vec::new()),
                node("add", "Add", Vec::new()),
            ],
            edges: vec![
                Edge {
                    from_node: "dv".to_string(),
                    from_port: "out".to_string(),
                    to_node: "add".to_string(),
                    to_port: "a".to_string(),
                },
                Edge {
                    from_node: "one".to_string(),
                    from_port: "out".to_string(),
                    to_node: "add".to_string(),
                    to_port: "b".to_string(),
                },
            ],
        };
        let library = vec![default_value, one, add];
        propagate_types(&mut playground, &library);
        assert_eq!(
            playground.node("add").unwrap().type_args,
            vec!["i32".to_string()]
        );
        assert_eq!(
            playground.node("one").unwrap().type_args,
            vec!["i32".to_string()]
        );
    }
}
