use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlInputElement, MouseEvent};
use yew::prelude::*;

mod api;
mod components;
mod context;
mod library;
mod persistence;

use components::graph_editor::GraphEditor;
use components::left_panel::LeftPanel;
use components::output_panel::OutputPanel;
use context::{DocAction, DocProvider, use_doc};

const MIN_OUTPUT_HEIGHT: f64 = 80.0;
const MIN_GRAPH_HEIGHT: f64 = 120.0;
const DEFAULT_OUTPUT_HEIGHT: f64 = 256.0;

#[derive(Clone)]
struct Resizing {
    start_y: f64,
    start_height: f64,
}

#[function_component(App)]
fn app() -> Html {
    let output_height = use_state(|| DEFAULT_OUTPUT_HEIGHT);
    let resizing = use_state(|| None::<Resizing>);
    let main_ref = use_node_ref();

    let on_divider_down = {
        let resizing = resizing.clone();
        let output_height = output_height.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            resizing.set(Some(Resizing {
                start_y: event.client_y() as f64,
                start_height: *output_height,
            }));
        })
    };

    // Track the pointer on the window while dragging the divider, so the drag
    // keeps working even when the cursor leaves the thin handle.
    {
        let resizing = resizing.clone();
        let output_height = output_height.clone();
        let main_ref = main_ref.clone();
        let active = resizing.is_some();
        use_effect_with_deps(
            move |active: &bool| {
                let listeners = active.then(|| {
                    let resizing_move = resizing.clone();
                    let output_height = output_height.clone();
                    let main_ref = main_ref.clone();
                    let window = web_sys::window().expect("no window");
                    let on_move =
                        gloo::events::EventListener::new(&window, "mousemove", move |event| {
                            let Some(state) = (*resizing_move).clone() else {
                                return;
                            };
                            let Some(event) = event.dyn_ref::<MouseEvent>() else {
                                return;
                            };
                            let max = main_ref
                                .cast::<Element>()
                                .map(|main| {
                                    main.get_bounding_client_rect().height() - MIN_GRAPH_HEIGHT
                                })
                                .unwrap_or(600.0)
                                .max(MIN_OUTPUT_HEIGHT);
                            let dy = event.client_y() as f64 - state.start_y;
                            output_height
                                .set((state.start_height - dy).clamp(MIN_OUTPUT_HEIGHT, max));
                        });
                    let on_up = gloo::events::EventListener::new(&window, "mouseup", move |_| {
                        resizing.set(None)
                    });
                    (on_move, on_up)
                });
                move || drop(listeners)
            },
            active,
        );
    }

    let divider_class = if resizing.is_some() {
        classes!("h-1.5", "shrink-0", "cursor-row-resize", "bg-accent")
    } else {
        classes!(
            "h-1.5",
            "shrink-0",
            "cursor-row-resize",
            "bg-line",
            "hover:bg-accent"
        )
    };
    let height = *output_height;

    html! {
        <DocProvider>
            <div class="h-screen flex flex-col">
                <Header />
                <div class="flex-1 flex min-h-0">
                    <aside class="w-1/3 min-w-[320px] max-w-[520px] border-r border-line bg-surface">
                        <LeftPanel />
                    </aside>
                    <main ref={main_ref} class="flex-1 flex flex-col min-w-0">
                        <div class="flex-1 min-h-0">
                            <GraphEditor />
                        </div>
                        <div class={divider_class} onmousedown={on_divider_down} />
                        <div
                            class="shrink-0 bg-surface-deep text-ink-bright overflow-auto"
                            style={format!("height: {height}px")}
                        >
                            <OutputPanel />
                        </div>
                    </main>
                </div>
            </div>
        </DocProvider>
    }
}

#[function_component(Header)]
fn header() -> Html {
    let doc = use_doc();

    let on_export = {
        let doc = doc.clone();
        Callback::from(move |_| {
            let playground = library::effective_playground(&doc.playground);
            persistence::export(&playground, "playgraph.json");
        })
    };

    let on_import = {
        let doc = doc.clone();
        Callback::from(move |event: Event| {
            let input: HtmlInputElement = event.target_unchecked_into();
            if let Some(file) = persistence::first_file(&input) {
                let doc = doc.clone();
                persistence::import(file, move |text| {
                    if let Ok(playground) = serde_json::from_str(&text) {
                        doc.dispatch(DocAction::ReplacePlayground(playground));
                    }
                });
            }
        })
    };

    html! {
        <header class="h-12 shrink-0 flex items-center gap-4 px-4 bg-surface-deep text-ink-bright">
            <span class="font-semibold tracking-wide text-accent">{"Rust Playgraph"}</span>
            <span class="text-xs text-ink-dim hidden md:inline">
                {"write stages in Rust, compose them visually, run them with the real compiler"}
            </span>
            <div class="ml-auto flex items-center gap-2 text-xs">
                <label class="px-3 py-1.5 border border-line rounded hover:bg-surface-light cursor-pointer">
                    {"Import"}
                    <input type="file" accept="application/json,.json" class="hidden" onchange={on_import} />
                </label>
                <button class="px-3 py-1.5 rounded bg-accent text-surface-deep" onclick={on_export}>
                    {"Export graph"}
                </button>
            </div>
        </header>
    }
}

fn main() {
    let root = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("app"))
        .expect("#app element is missing");
    yew::Renderer::<App>::with_root(root).render();
}
