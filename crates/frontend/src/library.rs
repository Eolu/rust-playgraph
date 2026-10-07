//! The stage library shipped with the frontend, embedded at compile time.

use std::sync::OnceLock;

use playgraph_core::model::{Playground, StageDef};
use serde::Deserialize;

const LIBRARY_JSON: &str = include_str!("../assets/library.json");

#[derive(Deserialize)]
struct LibraryFile {
    groups: Vec<LibraryGroup>,
}

/// A named category of built-in stages.
#[derive(Deserialize)]
pub struct LibraryGroup {
    pub name: String,
    pub stages: Vec<StageDef>,
}

fn parsed() -> &'static Vec<LibraryGroup> {
    static GROUPS: OnceLock<Vec<LibraryGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        let file: LibraryFile =
            serde_json::from_str(LIBRARY_JSON).expect("assets/library.json is not valid");
        file.groups
    })
}

/// The built-in stages, grouped by category.
pub fn groups() -> &'static [LibraryGroup] {
    parsed()
}

/// All built-in stages, flattened.
pub fn stages() -> &'static [StageDef] {
    static FLAT: OnceLock<Vec<StageDef>> = OnceLock::new();
    FLAT.get_or_init(|| {
        parsed()
            .iter()
            .flat_map(|group| group.stages.iter().cloned())
            .collect()
    })
}

/// Find a built-in stage by name.
pub fn find(name: &str) -> Option<&'static StageDef> {
    stages().iter().find(|stage| stage.name == name)
}

/// Resolve a stage by name, preferring the user's own definitions.
pub fn resolve<'a>(user: &'a [StageDef], name: &str) -> Option<&'a StageDef> {
    user.iter()
        .find(|stage| stage.name == name)
        .or_else(|| find(name))
}

/// A copy of the playground with any library stages referenced by its nodes
/// added, so codegen and export are self-contained.
pub fn effective_playground(playground: &Playground) -> Playground {
    let mut stages = playground.stages.clone();
    for node in &playground.nodes {
        if !stages.iter().any(|stage| stage.name == node.stage)
            && let Some(library_stage) = find(&node.stage)
        {
            stages.push(library_stage.clone());
        }
    }
    Playground {
        stages,
        nodes: playground.nodes.clone(),
        edges: playground.edges.clone(),
        prelude: playground.prelude.clone(),
    }
}
