//! Shared document state for the whole editor.

use std::rc::Rc;

use playgraph_core::api::ExecuteResponse;
use playgraph_core::model::{Edge, GraphNode, Playground, StageDef};
use yew::prelude::*;

use crate::persistence::{StoredDoc, load_doc, save_doc};

/// The editable document. Kept in a reducer so every panel sees the same state.
#[derive(Clone, PartialEq, Default)]
pub struct DocState {
    pub playground: Playground,
    /// The node currently selected on the canvas, if any. Transient; not saved.
    pub selected: Option<String>,
}

pub enum DocAction {
    /// Insert or replace a stage definition. `previous` is the stage's name
    /// before editing, so node references can be re-pointed if the function is
    /// renamed.
    UpsertStage {
        previous: Option<String>,
        stage: StageDef,
    },
    RemoveStage(String),
    /// Select a node (or clear the selection with `None`).
    SelectNode(Option<String>),
    /// Point an existing node at a different stage (used when a node's
    /// definition is forked into a new stage).
    SetNodeStage {
        id: String,
        stage: String,
    },
    AddNode(GraphNode),
    MoveNode {
        id: String,
        x: f64,
        y: f64,
    },
    /// Set a node's concrete generic type arguments.
    SetNodeTypeArgs {
        id: String,
        args: Vec<String>,
    },
    AddEdge(Edge),
    RemoveEdge(usize),
    ClearGraph,
    /// Replace the outermost-scope code block.
    SetPrelude(String),
    /// Replace the whole document (used by JSON import).
    ReplacePlayground(Playground),
}

impl Reducible for DocState {
    type Action = DocAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut playground = self.playground.clone();
        let mut selected = self.selected.clone();
        // Moving nodes doesn't change types; skip propagation there. Selecting
        // a node changes nothing about the document either.
        let propagate = !matches!(
            action,
            DocAction::MoveNode { .. }
                | DocAction::ClearGraph
                | DocAction::SelectNode(_)
                | DocAction::SetPrelude(_)
        );
        match action {
            DocAction::SelectNode(id) => selected = id,
            DocAction::SetPrelude(code) => playground.prelude = code,
            DocAction::SetNodeStage { id, stage } => {
                if let Some(node) = playground.nodes.iter_mut().find(|node| node.id == id) {
                    node.stage = stage;
                    node.type_args.clear();
                }
            }
            DocAction::UpsertStage { previous, stage } => {
                let new_name = stage.name.clone();
                // Drop the old definition and re-point nodes when renamed.
                if let Some(previous) = previous
                    && previous != new_name
                {
                    playground
                        .stages
                        .retain(|existing| existing.name != previous);
                    for node in &mut playground.nodes {
                        if node.stage == previous {
                            node.stage = new_name.clone();
                        }
                    }
                }
                if let Some(existing) = playground
                    .stages
                    .iter_mut()
                    .find(|existing| existing.name == new_name)
                {
                    *existing = stage;
                } else {
                    playground.stages.push(stage);
                }
            }
            DocAction::RemoveStage(name) => {
                playground.stages.retain(|stage| stage.name != name);
                playground.nodes.retain(|node| node.stage != name);
                let valid_nodes: Vec<String> = playground
                    .nodes
                    .iter()
                    .map(|node| node.id.clone())
                    .collect();
                playground.edges.retain(|edge| {
                    valid_nodes.contains(&edge.from_node) && valid_nodes.contains(&edge.to_node)
                });
            }
            DocAction::ReplacePlayground(replacement) => playground = replacement,
            DocAction::AddNode(node) => playground.nodes.push(node),
            DocAction::SetNodeTypeArgs { id, args } => {
                if let Some(node) = playground.nodes.iter_mut().find(|node| node.id == id) {
                    node.type_args = args;
                }
            }
            DocAction::MoveNode { id, x, y } => {
                if let Some(node) = playground.nodes.iter_mut().find(|node| node.id == id) {
                    node.x = x;
                    node.y = y;
                }
            }
            DocAction::AddEdge(edge) => {
                let duplicate = playground.edges.iter().any(|existing| {
                    existing.from_node == edge.from_node
                        && existing.from_port == edge.from_port
                        && existing.to_node == edge.to_node
                        && existing.to_port == edge.to_port
                });
                if !duplicate {
                    playground.edges.push(edge);
                }
            }
            DocAction::RemoveEdge(index) => {
                if index < playground.edges.len() {
                    playground.edges.remove(index);
                }
            }
            DocAction::ClearGraph => {
                playground.nodes.clear();
                playground.edges.clear();
            }
        }
        if propagate {
            playgraph_core::propagate_types(&mut playground, crate::library::stages());
        }
        // Drop the selection if its node no longer exists.
        if let Some(id) = &selected
            && playground.node(id).is_none()
        {
            selected = None;
        }
        Rc::new(Self {
            playground,
            selected,
        })
    }
}

pub type DocContext = UseReducerHandle<DocState>;

/// Transient state for the "Run" action and its result.
#[derive(Clone, PartialEq, Default)]
pub struct RunState {
    pub running: bool,
    pub result: Option<ExecuteResponse>,
    pub transport_error: Option<String>,
}

pub enum RunAction {
    Start,
    Finish(ExecuteResponse),
    Fail(String),
}

impl Reducible for RunState {
    type Action = RunAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            RunAction::Start => Rc::new(Self {
                running: true,
                result: self.result.clone(),
                transport_error: None,
            }),
            RunAction::Finish(result) => Rc::new(Self {
                running: false,
                result: Some(result),
                transport_error: None,
            }),
            RunAction::Fail(error) => Rc::new(Self {
                running: false,
                result: self.result.clone(),
                transport_error: Some(error),
            }),
        }
    }
}

pub type RunContext = UseReducerHandle<RunState>;

#[derive(Properties, PartialEq)]
pub struct DocProviderProps {
    #[prop_or_default]
    pub children: Children,
}

#[function_component(DocProvider)]
pub fn doc_provider(props: &DocProviderProps) -> Html {
    let doc = use_reducer(|| DocState {
        playground: initial_playground(),
        selected: None,
    });
    let run = use_reducer(RunState::default);

    // Persist user changes to browser storage on every document change.
    {
        let playground = doc.playground.clone();
        use_effect_with_deps(
            |playground: &Playground| {
                save_playground(playground);
                || ()
            },
            playground,
        );
    }

    html! {
        <ContextProvider<DocContext> context={doc}>
            <ContextProvider<RunContext> context={run}>
                { props.children.clone() }
            </ContextProvider<RunContext>>
        </ContextProvider<DocContext>>
    }
}

/// The document to start from: whatever the user saved last time. Built-in
/// library stages are available separately, from the library menu.
fn initial_playground() -> Playground {
    let mut playground = match load_doc() {
        Some(saved) => Playground {
            stages: saved.stages,
            nodes: saved.nodes,
            edges: saved.edges,
            prelude: saved.prelude,
        },
        None => Playground::default(),
    };
    playgraph_core::propagate_types(&mut playground, crate::library::stages());
    playground
}

/// Persist the user's own stages, the graph, and the outer-scope code.
fn save_playground(playground: &Playground) {
    save_doc(&StoredDoc {
        stages: playground.stages.clone(),
        nodes: playground.nodes.clone(),
        edges: playground.edges.clone(),
        prelude: playground.prelude.clone(),
    });
}

/// Convenience accessor; panics if used outside [`DocProvider`].
#[yew::hook]
pub fn use_doc() -> DocContext {
    use_context::<DocContext>().expect("DocProvider is missing")
}

/// Convenience accessor for the run state.
#[yew::hook]
pub fn use_run() -> RunContext {
    use_context::<RunContext>().expect("DocProvider is missing")
}
