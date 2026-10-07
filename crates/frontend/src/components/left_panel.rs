//! The left sidebar: a tabbed panel switching between stage authoring and the
//! outermost-scope code block.

use yew::prelude::*;

use crate::components::rust_code::RustCodeEditor;
use crate::components::stage_editor::StageEditor;
use crate::context::{DocAction, use_doc};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Stages,
    Code,
}

fn tab_class(active: bool) -> Classes {
    if active {
        classes!(
            "flex-1",
            "px-3",
            "py-2",
            "text-sm",
            "font-semibold",
            "text-ink-bright",
            "border-b-2",
            "border-accent"
        )
    } else {
        classes!(
            "flex-1",
            "px-3",
            "py-2",
            "text-sm",
            "text-ink-dim",
            "border-b-2",
            "border-transparent",
            "hover:text-ink"
        )
    }
}

#[function_component(LeftPanel)]
pub fn left_panel() -> Html {
    let tab = use_state(|| Tab::Stages);

    let show_stages = {
        let tab = tab.clone();
        Callback::from(move |_| tab.set(Tab::Stages))
    };
    let show_code = {
        let tab = tab.clone();
        Callback::from(move |_| tab.set(Tab::Code))
    };

    html! {
        <div class="h-full flex flex-col">
            <div class="flex shrink-0 border-b border-line bg-surface-dark">
                <button class={tab_class(*tab == Tab::Stages)} onclick={show_stages}>{"Stages"}</button>
                <button class={tab_class(*tab == Tab::Code)} onclick={show_code}>{"Global code"}</button>
            </div>
            if *tab == Tab::Stages {
                <div class="flex-1 min-h-0 overflow-y-auto">
                    <StageEditor />
                </div>
            } else {
                <GlobalCode />
            }
        </div>
    }
}

#[function_component(GlobalCode)]
fn global_code() -> Html {
    let doc = use_doc();
    let on_change = {
        let doc = doc.clone();
        Callback::from(move |code: String| doc.dispatch(DocAction::SetPrelude(code)))
    };

    html! {
        <div class="h-full flex flex-col gap-2 p-4">
            <p class="text-xs text-ink-dim">
                {"Emitted at the outermost scope, before the stage definitions. Use it for imports, helper functions, types and constants."}
            </p>
            <RustCodeEditor
                class={classes!("flex-1", "min-h-0")}
                value={doc.playground.prelude.clone()}
                on_change={on_change}
                placeholder={"// use std::collections::HashMap;\n// fn helper(x: i32) -> i32 { x + 1 }"}
            />
        </div>
    }
}
