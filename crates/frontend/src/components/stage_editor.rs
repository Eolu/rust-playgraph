//! Author stages: a name, optional generics/dressings, structured inputs and
//! outputs, the function body, and metadata.

use playgraph_core::is_valid_ident;
use playgraph_core::model::{Cache, Eval, OutputDef, ParamDef, StageDef};
use web_sys::{DragEvent, HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use crate::components::param_list::ParamList;
use crate::components::rust_code::{RustCodeEditor, highlighted};
use crate::context::{DocAction, use_doc};

#[derive(Clone, PartialEq)]
struct Form {
    name: String,
    has_generics: bool,
    generics: String,
    has_where: bool,
    where_clause: String,
    is_async: bool,
    inputs: Vec<ParamDef>,
    outputs: Vec<OutputDef>,
    body: String,
    has_state: bool,
    state_type: String,
    eval: Eval,
    cache: Cache,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            name: String::new(),
            has_generics: false,
            generics: String::new(),
            has_where: false,
            where_clause: String::new(),
            is_async: false,
            inputs: vec![ParamDef::default()],
            outputs: vec![OutputDef::default()],
            body: String::new(),
            has_state: false,
            state_type: String::new(),
            eval: Eval::default(),
            cache: Cache::default(),
        }
    }
}

impl Form {
    fn from_stage(stage: &StageDef) -> Self {
        Self {
            name: stage.name.clone(),
            has_generics: !stage.generics.trim().is_empty(),
            generics: strip_angle_brackets(&stage.generics),
            has_where: !stage.where_clause.trim().is_empty(),
            where_clause: stage.where_clause.clone(),
            is_async: stage.is_async,
            inputs: stage.inputs.clone(),
            outputs: stage.outputs.clone(),
            body: stage.body.clone(),
            has_state: stage.state_type.is_some(),
            state_type: stage.state_type.clone().unwrap_or_default(),
            eval: stage.eval,
            cache: stage.cache,
        }
    }

    fn to_stage(&self) -> StageDef {
        StageDef {
            name: self.name.trim().to_string(),
            generics: if self.has_generics {
                strip_angle_brackets(&self.generics)
            } else {
                String::new()
            },
            where_clause: if self.has_where {
                self.where_clause.trim().to_string()
            } else {
                String::new()
            },
            is_async: self.is_async,
            inputs: self
                .inputs
                .iter()
                .filter(|param| !param.name.trim().is_empty())
                .cloned()
                .collect(),
            outputs: self
                .outputs
                .iter()
                .filter(|output| !output.name.trim().is_empty())
                .cloned()
                .collect(),
            body: self.body.clone(),
            eval: self.eval,
            cache: self.cache,
            state_type: if self.has_state && !self.state_type.trim().is_empty() {
                Some(self.state_type.trim().to_string())
            } else {
                None
            },
        }
    }
}

/// Accept `T`, `T, U`, `<T>`, or `<T, U>` and store without the brackets.
fn strip_angle_brackets(text: &str) -> String {
    let text = text.trim();
    let inner = if text.starts_with('<') && text.ends_with('>') {
        &text[1..text.len() - 1]
    } else {
        text
    };
    inner.trim().to_string()
}

/// A name for a forked stage that collides with nothing: `Name`, `NameCopy`,
/// `NameCopy2`, …
fn unique_stage_name(stages: &[StageDef], base: &str) -> String {
    let taken = |candidate: &str| {
        stages.iter().any(|stage| stage.name == candidate)
            || crate::library::find(candidate).is_some()
    };
    if !taken(base) {
        return base.to_string();
    }
    let mut index = 2;
    loop {
        let candidate = format!("{base}Copy");
        let candidate = if index == 2 {
            candidate
        } else {
            format!("{base}Copy{index}")
        };
        if !taken(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

#[function_component(StageEditor)]
pub fn stage_editor() -> Html {
    let doc = use_doc();
    let form = use_state(Form::default);
    let editing = use_state(|| None::<String>);
    let library_open = use_state(|| false);

    let on_name = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.name = event.target_unchecked_into::<HtmlInputElement>().value();
            form.set(next);
        })
    };

    let on_has_generics = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.has_generics = event.target_unchecked_into::<HtmlInputElement>().checked();
            form.set(next);
        })
    };

    let on_generics = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.generics = event.target_unchecked_into::<HtmlInputElement>().value();
            form.set(next);
        })
    };

    let on_has_where = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.has_where = event.target_unchecked_into::<HtmlInputElement>().checked();
            form.set(next);
        })
    };

    let on_where_clause = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.where_clause = event.target_unchecked_into::<HtmlInputElement>().value();
            form.set(next);
        })
    };

    let on_async = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.is_async = event.target_unchecked_into::<HtmlInputElement>().checked();
            form.set(next);
        })
    };

    let on_has_state = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.has_state = event.target_unchecked_into::<HtmlInputElement>().checked();
            form.set(next);
        })
    };

    let on_state_type = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.state_type = event.target_unchecked_into::<HtmlInputElement>().value();
            form.set(next);
        })
    };

    let on_body = {
        let form = form.clone();
        Callback::from(move |text: String| {
            let mut next = (*form).clone();
            next.body = text;
            form.set(next);
        })
    };

    let on_eval = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.eval = if event.target_unchecked_into::<HtmlSelectElement>().value() == "lazy" {
                Eval::Lazy
            } else {
                Eval::Urgent
            };
            form.set(next);
        })
    };

    let on_cache = {
        let form = form.clone();
        Callback::from(move |event: Event| {
            let mut next = (*form).clone();
            next.cache = match event
                .target_unchecked_into::<HtmlSelectElement>()
                .value()
                .as_str()
            {
                "last" => Cache::Last,
                "all" => Cache::All,
                _ => Cache::None,
            };
            form.set(next);
        })
    };

    // --- input list ---
    let on_input_change = {
        let form = form.clone();
        Callback::from(move |(index, param): (usize, ParamDef)| {
            let mut next = (*form).clone();
            if index < next.inputs.len() {
                next.inputs[index] = param;
            }
            form.set(next);
        })
    };
    let on_input_add = {
        let form = form.clone();
        Callback::from(move |_| {
            let mut next = (*form).clone();
            next.inputs.push(ParamDef::default());
            form.set(next);
        })
    };
    let on_input_remove = {
        let form = form.clone();
        Callback::from(move |index: usize| {
            let mut next = (*form).clone();
            if index < next.inputs.len() {
                next.inputs.remove(index);
            }
            form.set(next);
        })
    };

    // --- output list (ParamList works in ParamDef, converted back) ---
    let on_output_change = {
        let form = form.clone();
        Callback::from(move |(index, param): (usize, ParamDef)| {
            let mut next = (*form).clone();
            if index < next.outputs.len() {
                next.outputs[index] = OutputDef {
                    name: param.name,
                    type_name: param.type_name,
                };
            }
            form.set(next);
        })
    };
    let on_output_add = {
        let form = form.clone();
        Callback::from(move |_| {
            let mut next = (*form).clone();
            next.outputs.push(OutputDef::default());
            form.set(next);
        })
    };
    let on_output_remove = {
        let form = form.clone();
        Callback::from(move |index: usize| {
            let mut next = (*form).clone();
            if index < next.outputs.len() {
                next.outputs.remove(index);
            }
            form.set(next);
        })
    };

    let on_new = {
        let form = form.clone();
        let editing = editing.clone();
        Callback::from(move |_| {
            form.set(Form::default());
            editing.set(None);
        })
    };

    let can_save = is_valid_ident(&form.name);
    let on_save = {
        let doc = doc.clone();
        let form = form.clone();
        let editing = editing.clone();
        Callback::from(move |_| {
            let stage = form.to_stage();
            if !is_valid_ident(&stage.name) {
                return;
            }
            let name = stage.name.clone();
            doc.dispatch(DocAction::UpsertStage {
                previous: (*editing).clone(),
                stage,
            });
            editing.set(Some(name));
        })
    };

    // Fork the edited stage into a fresh definition and point the selected node
    // at it, leaving the original untouched.
    let on_save_as_new = {
        let doc = doc.clone();
        let form = form.clone();
        let editing = editing.clone();
        Callback::from(move |_| {
            let stage = form.to_stage();
            if !is_valid_ident(&stage.name) {
                return;
            }
            let name = unique_stage_name(&doc.playground.stages, &stage.name);
            let mut fork = stage;
            fork.name = name.clone();
            doc.dispatch(DocAction::UpsertStage {
                previous: None,
                stage: fork,
            });
            editing.set(Some(name.clone()));
            let mut next = (*form).clone();
            next.name = name.clone();
            form.set(next);
            if let Some(id) = doc.selected.clone() {
                doc.dispatch(DocAction::SetNodeStage { id, stage: name });
            }
        })
    };

    let load_stage = {
        let doc = doc.clone();
        let form = form.clone();
        let editing = editing.clone();
        Callback::from(move |stage: StageDef| {
            let is_user = doc
                .playground
                .stages
                .iter()
                .any(|existing| existing.name == stage.name);
            editing.set(is_user.then(|| stage.name.clone()));
            form.set(Form::from_stage(&stage));
        })
    };

    // Clicking a node on the canvas loads its stage into this editor.
    {
        let doc = doc.clone();
        let form = form.clone();
        let editing = editing.clone();
        let selected = doc.selected.clone();
        use_effect_with_deps(
            move |selected: &Option<String>| {
                if let Some(id) = selected
                    && let Some(node) = doc.playground.node(id)
                    && let Some(stage) =
                        crate::library::resolve(&doc.playground.stages, &node.stage)
                {
                    let is_user = doc
                        .playground
                        .stages
                        .iter()
                        .any(|existing| existing.name == node.stage);
                    editing.set(is_user.then(|| node.stage.clone()));
                    form.set(Form::from_stage(stage));
                }
                || ()
            },
            selected,
        );
    }

    let on_deselect = {
        let doc = doc.clone();
        Callback::from(move |_| doc.dispatch(DocAction::SelectNode(None)))
    };

    let remove_stage = {
        let doc = doc.clone();
        Callback::from(move |name: String| {
            doc.dispatch(DocAction::RemoveStage(name));
        })
    };

    let toggle_library = {
        let library_open = library_open.clone();
        Callback::from(move |_| library_open.set(!*library_open))
    };

    let output_params: Vec<ParamDef> = form
        .outputs
        .iter()
        .map(|output| ParamDef {
            name: output.name.clone(),
            type_name: output.type_name.clone(),
        })
        .collect();
    let preview = form.to_stage().header();

    let banner = match doc
        .selected
        .as_ref()
        .and_then(|id| doc.playground.node(id).map(|node| node.stage.clone()))
    {
        Some(stage_name) => {
            let is_user = doc
                .playground
                .stages
                .iter()
                .any(|existing| existing.name == stage_name);
            let on_deselect = on_deselect.clone();
            html! {
                <div class="flex items-center justify-between gap-2 p-2 rounded border border-accent bg-accent/10 text-xs">
                    <span class="truncate">
                        {"Editing node "}
                        <span class="font-mono text-ink-bright">{ &stage_name }</span>
                        { format!(" ({})", if is_user { "your stage" } else { "library stage" }) }
                    </span>
                    <button class="shrink-0 text-ink hover:underline" onclick={on_deselect}>{"deselect"}</button>
                </div>
            }
        }
        None => html! {},
    };

    html! {
        <div class="p-4 space-y-4">
            <div class="flex items-center justify-between">
                <h2 class="text-lg font-bold">{"Stage"}</h2>
                <button class="text-xs text-ink hover:underline" onclick={on_new}>{"new"}</button>
            </div>

            { banner }

            <div>
                <button
                    class="w-full flex items-center justify-between px-3 py-2 border border-line rounded text-sm hover:bg-surface-light"
                    onclick={toggle_library}
                >
                    <span>{"Library"}</span>
                    <span class="text-ink-dim text-xs">
                        { if *library_open { "hide".to_string() } else { format!("{} stages", crate::library::stages().len()) } }
                    </span>
                </button>
                if *library_open {
                    <div class="mt-2 border border-line rounded p-2 bg-surface-dark max-h-80 overflow-y-auto space-y-2">
                        { crate::library::groups().iter().map(|group| html! {
                            <details open=true>
                                <summary class="cursor-pointer select-none text-xs font-semibold text-ink-bright">
                                    { &group.name }
                                </summary>
                                <div class="mt-1 space-y-1">
                                    { group.stages.iter().map(|stage| {
                                        let stage = stage.clone();
                                        let name = stage.name.clone();
                                        let drag_name = name.clone();
                                        let on_drag = Callback::from(move |event: DragEvent| {
                                            if let Some(data) = event.data_transfer() {
                                                let _ = data.set_data("text/plain", &drag_name);
                                            }
                                        });
                                        let load = load_stage.clone();
                                        let stage_for_load = stage.clone();
                                        html! {
                                            <div
                                                key={name.clone()}
                                                draggable="true"
                                                ondragstart={on_drag}
                                                class="flex items-center justify-between gap-2 px-2 py-1 rounded border border-transparent hover:border-line hover:bg-surface-light cursor-grab"
                                            >
                                                <div class="min-w-0">
                                                    <div class="font-mono text-xs truncate">{ &stage.name }</div>
                                                    <div class="text-[10px] text-ink-dim truncate">{ stage.header() }</div>
                                                </div>
                                                <button
                                                    class="shrink-0 text-[10px] px-1.5 py-0.5 border border-line rounded hover:bg-surface-light"
                                                    onclick={load.reform(move |_| stage_for_load.clone())}
                                                >{"edit"}</button>
                                            </div>
                                        }
                                    }).collect::<Html>() }
                                </div>
                            </details>
                        }).collect::<Html>() }
                    </div>
                }
            </div>

            <div>
                <label class="block text-xs font-medium text-ink-dim mb-1">{"Name"}</label>
                <input
                    type="text"
                    class="w-full p-2 border rounded font-mono text-sm"
                    placeholder="StageName"
                    value={form.name.clone()}
                    onchange={on_name}
                />
            </div>

            <label class="flex items-center gap-2 text-xs text-ink">
                <input type="checkbox" checked={form.is_async} onchange={on_async} />
                {"async fn"}
            </label>

            <div>
                <label class="flex items-center gap-2 text-xs text-ink mb-1">
                    <input type="checkbox" checked={form.has_generics} onchange={on_has_generics} />
                    {"generic"}
                </label>
                if form.has_generics {
                    <input
                        type="text"
                        class="w-full p-2 border rounded font-mono text-sm"
                        placeholder="T  ·  T, U"
                        value={form.generics.clone()}
                        onchange={on_generics}
                    />
                }
            </div>

            <div>
                <label class="flex items-center gap-2 text-xs text-ink mb-1">
                    <input type="checkbox" checked={form.has_where} onchange={on_has_where} />
                    {"where clause"}
                </label>
                if form.has_where {
                    <input
                        type="text"
                        class="w-full p-2 border rounded font-mono text-sm"
                        placeholder="T: Clone + Send"
                        value={form.where_clause.clone()}
                        onchange={on_where_clause}
                    />
                }
            </div>

            <ParamList
                title="Inputs"
                type_placeholder="i32"
                params={form.inputs.clone()}
                on_change={on_input_change}
                on_remove={on_input_remove}
                on_add={on_input_add}
            />

            <ParamList
                title="Outputs"
                type_placeholder="i32"
                params={output_params}
                on_change={on_output_change}
                on_remove={on_output_remove}
                on_add={on_output_add}
            />

            <div>
                <label class="flex items-center gap-2 text-xs text-ink mb-1">
                    <input type="checkbox" checked={form.has_state} onchange={on_has_state} />
                    {"has state"}
                </label>
                if form.has_state {
                    <input
                        type="text"
                        class="w-full p-2 border rounded font-mono text-sm"
                        placeholder="state type, e.g. u32"
                        value={form.state_type.clone()}
                        onchange={on_state_type}
                    />
                }
            </div>

            <div class="grid grid-cols-2 gap-2">
                <div>
                    <label class="block text-xs font-medium text-ink-dim mb-1">{"Evaluation"}</label>
                    <select class="w-full p-2 border rounded text-sm" onchange={on_eval}
                        value={match form.eval { Eval::Lazy => "lazy", Eval::Urgent => "urgent" }}>
                        <option value="urgent">{"urgent"}</option>
                        <option value="lazy">{"lazy"}</option>
                    </select>
                </div>
                <div>
                    <label class="block text-xs font-medium text-ink-dim mb-1">{"Caching"}</label>
                    <select class="w-full p-2 border rounded text-sm" onchange={on_cache}
                        value={match form.cache { Cache::None => "none", Cache::Last => "last", Cache::All => "all" }}>
                        <option value="none">{"none"}</option>
                        <option value="last">{"cache_last"}</option>
                        <option value="all">{"cache_all"}</option>
                    </select>
                </div>
            </div>

            <div>
                <label class="block text-xs font-medium text-ink-dim mb-1">{"Body"}</label>
                <RustCodeEditor
                    value={form.body.clone()}
                    on_change={on_body}
                    placeholder="value * 2"
                />
            </div>

            <div class="p-2 rounded bg-surface-light text-xs font-mono break-all">
                { highlighted(&preview) }
            </div>

            <div class="grid grid-cols-2 gap-2">
                <button
                    class="w-full bg-accent text-surface-deep px-4 py-2 rounded disabled:opacity-40"
                    disabled={!can_save}
                    onclick={on_save}
                >
                    {"Save stage"}
                </button>
                <button
                    class="w-full border border-line px-4 py-2 rounded hover:bg-surface-light disabled:opacity-40"
                    disabled={!can_save}
                    onclick={on_save_as_new}
                >
                    {"Save as new"}
                </button>
            </div>
            <p class="text-[10px] text-ink-dim">
                {"Save updates this stage for every node using it. Save as new forks a copy and points the selected node at it."}
            </p>

            <div class="pt-2 border-t">
                <h3 class="text-sm font-semibold mb-2">{"Saved stages"}</h3>
                if doc.playground.stages.is_empty() {
                    <p class="text-xs text-ink-dim">{"No stages yet. Save one, then drag it into the graph."}</p>
                } else {
                    <div class="space-y-2">
                        { doc.playground.stages.iter().map(|stage| {
                            let stage = stage.clone();
                            let name = stage.name.clone();
                            let load = load_stage.clone();
                            let remove = remove_stage.clone();
                            let stage_for_drag = stage.clone();
                            let on_drag = Callback::from(move |event: DragEvent| {
                                if let Some(data) = event.data_transfer() {
                                    let _ = data.set_data("text/plain", &stage_for_drag.name);
                                }
                            });
                            let name_for_remove = name.clone();
                            let stage_for_load = stage.clone();
                            html! {
                                <div
                                    key={name.clone()}
                                    class="flex items-center justify-between p-2 border rounded cursor-grab hover:bg-surface-light"
                                    draggable="true"
                                    ondragstart={on_drag}
                                >
                                    <div class="min-w-0">
                                        <div class="font-mono text-sm truncate">{ &name }</div>
                                        <div class="text-[10px] text-ink-dim truncate">
                                            { stage.header() }
                                        </div>
                                    </div>
                                    <div class="flex gap-1 shrink-0">
                                        <button
                                            class="text-xs px-2 py-1 border rounded hover:bg-surface-light"
                                            onclick={load.reform(move |_| stage_for_load.clone())}
                                        >{"edit"}</button>
                                        <button
                                            class="text-xs px-2 py-1 border rounded text-danger hover:bg-danger/10"
                                            onclick={remove.reform(move |_| name_for_remove.clone())}
                                        >{"delete"}</button>
                                    </div>
                                </div>
                            }
                        }).collect::<Html>() }
                    </div>
                }
            </div>
        </div>
    }
}
