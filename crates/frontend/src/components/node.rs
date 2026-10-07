//! A single node card with input/output ports.

use playgraph_core::Port;
use web_sys::{HtmlInputElement, MouseEvent};
use yew::prelude::*;

pub const NODE_WIDTH: f64 = 200.0;
pub const HEADER_HEIGHT: f64 = 34.0;
pub const ROW_HEIGHT: f64 = 26.0;
pub const BODY_PAD: f64 = 6.0;

/// Vertical centre of a port row, relative to the canvas.
pub fn row_center(node_y: f64, row: usize) -> f64 {
    node_y + HEADER_HEIGHT + BODY_PAD + row as f64 * ROW_HEIGHT + ROW_HEIGHT / 2.0
}

#[derive(Properties, PartialEq)]
pub struct NodeProps {
    pub id: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    /// Type parameter names of the stage (empty for non-generic stages).
    pub type_params: Vec<String>,
    /// Concrete type arguments chosen for this node.
    pub type_args: Vec<String>,
    /// Header drag: (node id, client x, client y).
    pub on_drag_start: Callback<(String, f64, f64)>,
    /// Output port pressed: (node id, output port name).
    pub on_port_start: Callback<(String, String)>,
    /// Input port released: (node id, input port name).
    pub on_port_end: Callback<(String, String)>,
    /// Type argument changed: (node id, all type arguments).
    pub on_type_args: Callback<(String, Vec<String>)>,
    /// The node was clicked (selects it for editing).
    pub on_select: Callback<String>,
    /// Whether this node is the current selection.
    pub selected: bool,
}

#[function_component(NodeView)]
pub fn node_view(props: &NodeProps) -> Html {
    let on_header = {
        let on_drag_start = props.on_drag_start.clone();
        let id = props.id.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            on_drag_start.emit((id.clone(), event.client_x() as f64, event.client_y() as f64));
        })
    };

    let on_select = {
        let on_select = props.on_select.clone();
        let id = props.id.clone();
        Callback::from(move |event: MouseEvent| {
            event.stop_propagation();
            on_select.emit(id.clone());
        })
    };

    // Keep mousedowns inside a node from starting a canvas pan.
    let stop_mousedown = Callback::from(|event: MouseEvent| event.stop_propagation());

    let border = if props.selected {
        "border-accent ring-2 ring-accent"
    } else {
        "border-line"
    };

    html! {
        <div
            class="absolute select-none"
            style={format!("left: {}px; top: {}px; width: {}px", props.x, props.y, NODE_WIDTH)}
            onclick={on_select}
            onmousedown={stop_mousedown}
        >
            <div class={format!("rounded-lg border {border} bg-surface-light shadow-lg overflow-hidden")}>
                <div
                    class="px-3 py-2 text-sm font-semibold text-ink-bright bg-surface border-b border-line cursor-move"
                    onmousedown={on_header}
                >
                    { &props.name }
                </div>
                if !props.type_params.is_empty() {
                    <div class="px-2 py-1 border-b border-line bg-surface-dark space-y-1">
                        { props.type_params.iter().enumerate().map(|(index, param)| {
                            let on_type_args = props.on_type_args.clone();
                            let id = props.id.clone();
                            let current = props.type_args.clone();
                            let value = props.type_args.get(index).cloned().unwrap_or_default();
                            html! {
                                <div key={index} class="flex items-center gap-1">
                                    <span class="w-6 text-[10px] font-mono text-ink-dim truncate">{ param }</span>
                                    <input
                                        type="text"
                                        class="flex-1 min-w-0 p-0.5 border rounded font-mono text-[10px]"
                                        placeholder="_"
                                        value={value}
                                        onchange={Callback::from(move |event: Event| {
                                            let text = event.target_unchecked_into::<HtmlInputElement>().value();
                                            let mut args = current.clone();
                                            while args.len() <= index {
                                                args.push(String::new());
                                            }
                                            args[index] = text;
                                            on_type_args.emit((id.clone(), args));
                                        })}
                                    />
                                </div>
                            }
                        }).collect::<Html>() }
                    </div>
                }
                <div class="relative py-1" style={format!("min-height: {}px", ROW_HEIGHT)}>
                    { rows(props, false) }
                    { rows(props, true) }
                </div>
            </div>
        </div>
    }
}

fn rows(props: &NodeProps, outputs: bool) -> Html {
    let ports = if outputs {
        &props.outputs
    } else {
        &props.inputs
    };
    ports
        .iter()
        .map(|port| {
            let on_start = props.on_port_start.clone();
            let on_end = props.on_port_end.clone();
            let id_start = props.id.clone();
            let id_end = props.id.clone();
            let name = port.name.clone();
            let ty = port.type_name.clone();
            if outputs {
                html! {
                    <div class="relative flex items-center justify-end px-3" style={format!("height: {}px", ROW_HEIGHT)}>
                        <span class="mr-2 text-[11px] text-ink font-mono truncate">{ &name }</span>
                        <span class="text-[10px] text-ink-dim font-mono">{ format!(": {ty}") }</span>
                        <span
                            class="absolute -right-[6px] top-1/2 -translate-y-1/2 w-3 h-3 rounded-full bg-success border border-success cursor-crosshair"
                            onmousedown={
                                let name = name.clone();
                                move |event: MouseEvent| {
                                    event.stop_propagation();
                                    on_start.emit((id_start.clone(), name.clone()));
                                }
                            }
                        />
                    </div>
                }
            } else {
                html! {
                    <div class="relative flex items-center px-3" style={format!("height: {}px", ROW_HEIGHT)}>
                        <span
                            class="absolute -left-[6px] top-1/2 -translate-y-1/2 w-3 h-3 rounded-full bg-info border border-info cursor-crosshair"
                            onmouseup={
                                let name = name.clone();
                                move |event: MouseEvent| {
                                    event.stop_propagation();
                                    on_end.emit((id_end.clone(), name.clone()));
                                }
                            }
                        />
                        <span class="text-[10px] text-ink-dim font-mono">{ format!("{ty}: ") }</span>
                        <span class="text-[11px] text-ink font-mono truncate">{ &name }</span>
                    </div>
                }
            }
        })
        .collect()
}
