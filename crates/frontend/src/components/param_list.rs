//! An editable list of `name`/`Type` rows, used for stage inputs and outputs.

use playgraph_core::model::ParamDef;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ParamListProps {
    pub title: AttrValue,
    pub type_placeholder: AttrValue,
    pub params: Vec<ParamDef>,
    pub on_change: Callback<(usize, ParamDef)>,
    pub on_remove: Callback<usize>,
    pub on_add: Callback<()>,
}

#[function_component(ParamList)]
pub fn param_list(props: &ParamListProps) -> Html {
    html! {
        <div>
            <div class="flex items-center justify-between mb-1">
                <label class="text-xs font-medium text-ink-dim">{ props.title.clone() }</label>
                <button
                    type="button"
                    class="text-xs text-accent hover:underline"
                    onclick={props.on_add.reform(|_| ())}
                >
                    {"+ add"}
                </button>
            </div>
            <div class="space-y-1">
                { props.params.iter().enumerate().map(|(index, param)| {
                    let on_change_name = props.on_change.clone();
                    let on_change_type = props.on_change.clone();
                    let on_remove = props.on_remove.clone();
                    let name_original = param.name.clone();
                    let type_original = param.type_name.clone();
                    html! {
                        <div key={index} class="flex gap-1 items-center">
                            <input
                                type="text"
                                class="w-1/3 p-1.5 border rounded font-mono text-xs"
                                placeholder="name"
                                value={param.name.clone()}
                                onchange={Callback::from(move |event: Event| {
                                    let value = event.target_unchecked_into::<HtmlInputElement>().value();
                                    on_change_name.emit((index, ParamDef {
                                        name: value,
                                        type_name: type_original.clone(),
                                    }));
                                })}
                            />
                            <input
                                type="text"
                                class="flex-1 p-1.5 border rounded font-mono text-xs"
                                placeholder={props.type_placeholder.clone()}
                                value={param.type_name.clone()}
                                onchange={Callback::from(move |event: Event| {
                                    let value = event.target_unchecked_into::<HtmlInputElement>().value();
                                    on_change_type.emit((index, ParamDef {
                                        name: name_original.clone(),
                                        type_name: value,
                                    }));
                                })}
                            />
                            <button
                                type="button"
                                class="px-2 py-1 border rounded text-danger hover:bg-danger/10 text-xs"
                                onclick={on_remove.reform(move |_| index)}
                            >
                                {"×"}
                            </button>
                        </div>
                    }
                }).collect::<Html>() }
            </div>
        </div>
    }
}
