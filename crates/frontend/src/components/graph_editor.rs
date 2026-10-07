//! The canvas: place nodes, wire ports, and run the graph.

use std::collections::HashMap;

use playgraph_core::StageSignature;
use playgraph_core::model::{Edge, GraphNode, StageDef};
use uuid::Uuid;
use web_sys::{DragEvent, Element, MouseEvent};
use yew::prelude::*;

use crate::components::node::{NODE_WIDTH, NodeView, row_center};
use crate::context::{DocAction, RunAction, use_doc, use_run};

#[derive(Clone)]
struct Pending {
    from_node: String,
    from_port: String,
    x: f64,
    y: f64,
}

#[derive(Clone)]
struct Dragging {
    node: String,
    start_x: f64,
    start_y: f64,
    node_x: f64,
    node_y: f64,
}

#[derive(Clone)]
struct Panning {
    start_x: f64,
    start_y: f64,
    pan_x: f64,
    pan_y: f64,
}

#[function_component(GraphEditor)]
pub fn graph_editor() -> Html {
    let doc = use_doc();
    let run = use_run();
    let container = use_node_ref();
    let dragging = use_state(|| None::<Dragging>);
    let pending = use_state(|| None::<Pending>);
    // View offset in pixels; nodes are drawn at world position + pan. Unbounded,
    // so the canvas can represent arbitrarily large graphs.
    let pan = use_state(|| (0.0_f64, 0.0_f64));
    let panning = use_state(|| None::<Panning>);
    let pan_moved = use_state(|| false);
    let suppress_click = use_state(|| false);

    // Ports are computed per node, so generic stages can be instantiated.
    let node_signatures = use_memo(
        |(stages, nodes): &(Vec<StageDef>, Vec<GraphNode>)| {
            let mut map: HashMap<String, StageSignature> = HashMap::new();
            for node in nodes {
                if let Some(stage) = crate::library::resolve(stages, &node.stage) {
                    map.insert(node.id.clone(), stage.node_signature(&node.type_args));
                }
            }
            map
        },
        (doc.playground.stages.clone(), doc.playground.nodes.clone()),
    );

    let relative = {
        let container = container.clone();
        move |client_x: f64, client_y: f64| -> (f64, f64) {
            if let Some(element) = container.cast::<Element>() {
                let rect = element.get_bounding_client_rect();
                (client_x - rect.left(), client_y - rect.top())
            } else {
                (client_x, client_y)
            }
        }
    };

    let on_drop = {
        let doc = doc.clone();
        let relative = relative.clone();
        let pan = pan.clone();
        Callback::from(move |event: DragEvent| {
            event.prevent_default();
            let Some(data) = event.data_transfer() else {
                return;
            };
            let Ok(stage) = data.get_data("text/plain") else {
                return;
            };
            if crate::library::resolve(&doc.playground.stages, &stage).is_none() {
                return;
            }
            let (x, y) = relative(event.client_x() as f64, event.client_y() as f64);
            let (pan_x, pan_y) = *pan;
            doc.dispatch(DocAction::AddNode(GraphNode {
                id: Uuid::new_v4().to_string(),
                stage,
                type_args: Vec::new(),
                x: x - pan_x - NODE_WIDTH / 2.0,
                y: y - pan_y - 16.0,
            }));
        })
    };

    let on_drag_over = Callback::from(|event: DragEvent| event.prevent_default());

    let on_drag_start = {
        let dragging = dragging.clone();
        let doc = doc.clone();
        Callback::from(move |(id, client_x, client_y): (String, f64, f64)| {
            if let Some(node) = doc.playground.node(&id) {
                dragging.set(Some(Dragging {
                    node: id.clone(),
                    start_x: client_x,
                    start_y: client_y,
                    node_x: node.x,
                    node_y: node.y,
                }));
            }
        })
    };

    let on_port_start = {
        let pending = pending.clone();
        Callback::from(move |(node, port): (String, String)| {
            pending.set(Some(Pending {
                from_node: node,
                from_port: port,
                x: 0.0,
                y: 0.0,
            }));
        })
    };

    let on_port_end = {
        let doc = doc.clone();
        let pending = pending.clone();
        let node_signatures = node_signatures.clone();
        Callback::from(move |(to_node_id, to_port): (String, String)| {
            let Some(connection) = (*pending).clone() else {
                return;
            };
            if connection.from_node == to_node_id {
                return;
            }
            let (Some(from_node), Some(to_node)) = (
                doc.playground.node(&connection.from_node).cloned(),
                doc.playground.node(&to_node_id).cloned(),
            ) else {
                return;
            };
            let (Some(from_stage), Some(to_stage)) = (
                crate::library::resolve(&doc.playground.stages, &from_node.stage),
                crate::library::resolve(&doc.playground.stages, &to_node.stage),
            ) else {
                return;
            };
            let (Some(from_sig), Some(to_sig)) = (
                node_signatures.get(&connection.from_node),
                node_signatures.get(&to_node_id),
            ) else {
                return;
            };
            let Some(output) = from_sig
                .outputs
                .iter()
                .find(|port| port.name == connection.from_port)
            else {
                return;
            };
            let Some(input) = to_sig.inputs.iter().find(|port| port.name == to_port) else {
                return;
            };

            if output.type_name != input.type_name {
                // Try to infer the generic type arguments from the connection.
                let Some(unified) = playgraph_core::unify_ports(
                    &from_stage.type_params(),
                    &from_node.type_args,
                    &output.type_name,
                    &to_stage.type_params(),
                    &to_node.type_args,
                    &input.type_name,
                ) else {
                    return;
                };
                if unified.from != from_node.type_args {
                    doc.dispatch(DocAction::SetNodeTypeArgs {
                        id: from_node.id.clone(),
                        args: unified.from,
                    });
                }
                if unified.to != to_node.type_args {
                    doc.dispatch(DocAction::SetNodeTypeArgs {
                        id: to_node.id.clone(),
                        args: unified.to,
                    });
                }
            }

            doc.dispatch(DocAction::AddEdge(Edge {
                from_node: connection.from_node.clone(),
                from_port: connection.from_port.clone(),
                to_node: to_node_id,
                to_port,
            }));
            pending.set(None);
        })
    };

    let on_mouse_move = {
        let dragging = dragging.clone();
        let pending = pending.clone();
        let panning = panning.clone();
        let pan = pan.clone();
        let pan_moved = pan_moved.clone();
        let doc = doc.clone();
        let relative = relative.clone();
        Callback::from(move |event: MouseEvent| {
            if let Some(state) = (*dragging).clone() {
                let node_x = state.node_x + event.client_x() as f64 - state.start_x;
                let node_y = state.node_y + event.client_y() as f64 - state.start_y;
                doc.dispatch(DocAction::MoveNode {
                    id: state.node.clone(),
                    x: node_x,
                    y: node_y,
                });
            }
            if let Some(state) = (*pending).clone() {
                let (x, y) = relative(event.client_x() as f64, event.client_y() as f64);
                pending.set(Some(Pending { x, y, ..state }));
            }
            if let Some(state) = (*panning).clone() {
                let dx = event.client_x() as f64 - state.start_x;
                let dy = event.client_y() as f64 - state.start_y;
                if dx.abs() > 3.0 || dy.abs() > 3.0 {
                    pan_moved.set(true);
                }
                pan.set((state.pan_x + dx, state.pan_y + dy));
            }
        })
    };

    let on_mouse_up = {
        let dragging = dragging.clone();
        let panning = panning.clone();
        let pan_moved = pan_moved.clone();
        let suppress_click = suppress_click.clone();
        Callback::from(move |_| {
            // A pan ends with a synthetic click; don't let it clear the
            // selection or a pending connection.
            if panning.is_some() && *pan_moved {
                suppress_click.set(true);
            }
            dragging.set(None);
            panning.set(None);
            pan_moved.set(false);
        })
    };

    // Dragging empty canvas space pans the view.
    let on_background_mousedown = {
        let panning = panning.clone();
        let pan = pan.clone();
        let pan_moved = pan_moved.clone();
        Callback::from(move |event: MouseEvent| {
            if event.button() != 0 {
                return;
            }
            let (pan_x, pan_y) = *pan;
            pan_moved.set(false);
            panning.set(Some(Panning {
                start_x: event.client_x() as f64,
                start_y: event.client_y() as f64,
                pan_x,
                pan_y,
            }));
        })
    };

    let on_run = {
        let doc = doc.clone();
        let run = run.clone();
        Callback::from(move |_| {
            let playground = crate::library::effective_playground(&doc.playground);
            run.dispatch(RunAction::Start);
            let run = run.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match crate::api::execute(&playground).await {
                    Ok(result) => run.dispatch(RunAction::Finish(result)),
                    Err(error) => run.dispatch(RunAction::Fail(error)),
                }
            });
        })
    };

    let on_clear = {
        let doc = doc.clone();
        Callback::from(move |_| doc.dispatch(DocAction::ClearGraph))
    };

    // Clicking anywhere that is not the end of a connection cancels it and
    // clears the node selection. A pan is followed by a click we ignore.
    let on_background_click = {
        let pending = pending.clone();
        let doc = doc.clone();
        let suppress_click = suppress_click.clone();
        Callback::from(move |_| {
            if *suppress_click {
                suppress_click.set(false);
                return;
            }
            pending.set(None);
            doc.dispatch(DocAction::SelectNode(None));
        })
    };

    let on_reset_view = {
        let pan = pan.clone();
        Callback::from(move |_| pan.set((0.0, 0.0)))
    };

    let on_select = {
        let doc = doc.clone();
        Callback::from(move |id: String| doc.dispatch(DocAction::SelectNode(Some(id))))
    };

    let on_edge_click = {
        let doc = doc.clone();
        Callback::from(move |index: usize| doc.dispatch(DocAction::RemoveEdge(index)))
    };

    let on_type_args = {
        let doc = doc.clone();
        Callback::from(move |(id, args): (String, Vec<String>)| {
            doc.dispatch(DocAction::SetNodeTypeArgs { id, args });
        })
    };

    // End a drag/pan even when the mouse is released outside the canvas (or
    // over a port that stops the event from bubbling).
    {
        let dragging = dragging.clone();
        let panning = panning.clone();
        let pan_moved = pan_moved.clone();
        let suppress_click = suppress_click.clone();
        let active = panning.is_some() || dragging.is_some();
        use_effect_with_deps(
            move |active: &bool| {
                let listener = active.then(|| {
                    let dragging = dragging.clone();
                    let panning = panning.clone();
                    let pan_moved = pan_moved.clone();
                    let suppress_click = suppress_click.clone();
                    let window = web_sys::window().expect("no window");
                    gloo::events::EventListener::new(&window, "mouseup", move |_| {
                        if panning.is_some() && *pan_moved {
                            suppress_click.set(true);
                        }
                        dragging.set(None);
                        panning.set(None);
                        pan_moved.set(false);
                    })
                });
                move || drop(listener)
            },
            active,
        );
    }

    let node_of = |id: &str| doc.playground.node(id);
    let (pan_x, pan_y) = *pan;

    html! {
        <div class="h-full flex flex-col">
            <div class="flex items-center justify-between px-4 py-2 border-b border-line bg-surface">
                <h2 class="text-base font-bold">{"Graph"}</h2>
                <div class="flex gap-2">
                    <button
                        class="text-xs px-3 py-1.5 border rounded hover:bg-surface-light"
                        onclick={on_reset_view}
                    >{"Reset view"}</button>
                    <button
                        class="text-xs px-3 py-1.5 border rounded hover:bg-surface-light"
                        onclick={on_clear}
                    >{"Clear"}</button>
                    <button
                        class="text-xs px-4 py-1.5 rounded bg-success text-surface-deep disabled:opacity-40 flex items-center gap-2"
                        disabled={run.running}
                        onclick={on_run}
                    >
                        { if run.running { "Running…" } else { "Run graph" } }
                    </button>
                </div>
            </div>

            <div
                ref={container}
                class={classes!(
                    "relative", "flex-1", "overflow-hidden", "bg-surface", "select-none",
                    if panning.is_some() { "cursor-grabbing" } else { "cursor-grab" }
                )}
                style={format!(
                    "background-image: radial-gradient(#292e42 1px, transparent 1px); background-size: 22px 22px; background-position: {}px {}px;",
                    pan_x, pan_y
                )}
                ondrop={on_drop}
                ondragover={on_drag_over}
                onmousedown={on_background_mousedown}
                onmousemove={on_mouse_move}
                onmouseup={on_mouse_up}
                onclick={on_background_click}
            >
                <svg class="absolute inset-0 w-full h-full pointer-events-none">
                    { render_edges(&doc.playground, &node_signatures, (pan_x, pan_y), &on_edge_click) }
                    if let Some(state) = &*pending {
                        if let Some(node) = node_of(&state.from_node) {
                            if let Some(row) = output_row(&node_signatures, &state.from_node, &state.from_port) {
                                <line
                                    x1={format!("{}", node.x + pan_x + NODE_WIDTH)}
                                    y1={format!("{}", row_center(node.y + pan_y, row))}
                                    x2={format!("{}", state.x)}
                                    y2={format!("{}", state.y)}
                                    style="stroke: #7aa2f7; stroke-width: 2; stroke-dasharray: 5 4;"
                                />
                            }
                        }
                    }
                </svg>

                if doc.playground.nodes.is_empty() {
                    <div class="absolute inset-0 flex items-center justify-center text-ink-dim text-sm">
                        {"Drag a stage from the left to create a node"}
                    </div>
                }

                { doc.playground.nodes.iter().map(|node| {
                    let (inputs, outputs) = match node_signatures.get(&node.id) {
                        Some(signature) => (signature.inputs.clone(), signature.outputs.clone()),
                        None => (Vec::new(), Vec::new()),
                    };
                    let type_params = doc
                        .playground
                        .stage(&node.stage)
                        .map(|stage| stage.type_params())
                        .unwrap_or_default();
                    html! {
                        <NodeView
                            key={node.id.clone()}
                            id={node.id.clone()}
                            name={node.stage.clone()}
                            x={node.x + pan_x}
                            y={node.y + pan_y}
                            inputs={inputs}
                            outputs={outputs}
                            type_params={type_params}
                            type_args={node.type_args.clone()}
                            on_drag_start={on_drag_start.clone()}
                            on_port_start={on_port_start.clone()}
                            on_port_end={on_port_end.clone()}
                            on_type_args={on_type_args.clone()}
                            on_select={on_select.clone()}
                            selected={doc.selected.as_deref() == Some(node.id.as_str())}
                        />
                    }
                }).collect::<Html>() }
            </div>
        </div>
    }
}

fn output_row(
    node_signatures: &HashMap<String, StageSignature>,
    node_id: &str,
    port: &str,
) -> Option<usize> {
    let signature = node_signatures.get(node_id)?;
    let index = signature.outputs.iter().position(|p| p.name == port)?;
    Some(signature.inputs.len() + index)
}

fn render_edges(
    playground: &playgraph_core::Playground,
    node_signatures: &HashMap<String, StageSignature>,
    pan: (f64, f64),
    on_click: &Callback<usize>,
) -> Html {
    let (pan_x, pan_y) = pan;
    playground
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            let from = match playground.node(&edge.from_node) {
                Some(node) => node,
                None => return html! {},
            };
            let to = match playground.node(&edge.to_node) {
                Some(node) => node,
                None => return html! {},
            };
            let from_sig = match node_signatures.get(&edge.from_node) {
                Some(signature) => signature,
                None => return html! {},
            };
            let to_sig = match node_signatures.get(&edge.to_node) {
                Some(signature) => signature,
                None => return html! {},
            };
            let Some(out_index) = from_sig.outputs.iter().position(|p| p.name == edge.from_port)
            else {
                return html! {};
            };
            let Some(in_index) = to_sig.inputs.iter().position(|p| p.name == edge.to_port) else {
                return html! {};
            };
            let x1 = from.x + pan_x + NODE_WIDTH;
            let y1 = row_center(from.y + pan_y, from_sig.inputs.len() + out_index);
            let x2 = to.x + pan_x;
            let y2 = row_center(to.y + pan_y, in_index);
            let on_click = on_click.clone();
            html! {
                <g key={index}>
                    <line
                        x1={x1.to_string()} y1={y1.to_string()}
                        x2={x2.to_string()} y2={y2.to_string()}
                        style="stroke: #414868; stroke-width: 2;"
                    />
                    <line
                        x1={x1.to_string()} y1={y1.to_string()}
                        x2={x2.to_string()} y2={y2.to_string()}
                        style="stroke: transparent; stroke-width: 12; pointer-events: stroke; cursor: pointer;"
                        onmousedown={Callback::from(|event: MouseEvent| event.stop_propagation())}
                        onclick={Callback::from(move |_: MouseEvent| on_click.emit(index))}
                    />
                </g>
            }
        })
        .collect()
}
