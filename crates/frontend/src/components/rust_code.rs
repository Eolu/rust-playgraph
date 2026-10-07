//! Rust syntax highlighting: a read-only block and an editable body editor.
//!
//! The editor renders highlighted tokens in a `<pre>` behind a transparent
//! `<textarea>`, so text stays editable while the caret and selection show.

use playgraph_core::TokenKind;
use web_sys::{Element, HtmlTextAreaElement, InputEvent};
use yew::prelude::*;

fn class_of(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Plain => "",
        TokenKind::Comment => "tok-comment",
        TokenKind::Str => "tok-string",
        TokenKind::Number => "tok-number",
        TokenKind::Keyword => "tok-keyword",
        TokenKind::Type => "tok-type",
        TokenKind::Macro => "tok-macro",
        TokenKind::Attribute => "tok-attr",
        TokenKind::Lifetime => "tok-lifetime",
    }
}

/// Render `source` as highlighted inline nodes (safe: never raw HTML).
pub fn highlighted(source: &str) -> Html {
    playgraph_core::tokenize(source)
        .into_iter()
        .map(|(kind, text)| match class_of(kind) {
            "" => html! { { text } },
            class => html! { <span class={class}>{ text }</span> },
        })
        .collect::<Html>()
}

#[derive(Properties, PartialEq)]
pub struct RustCodeBlockProps {
    pub code: AttrValue,
    #[prop_or_default]
    pub class: Classes,
}

/// A read-only, syntax-highlighted code block.
#[function_component(RustCodeBlock)]
pub fn rust_code_block(props: &RustCodeBlockProps) -> Html {
    html! {
        <pre class={classes!("font-mono", "whitespace-pre-wrap", props.class.clone())}>
            <code>{ highlighted(&props.code) }</code>
        </pre>
    }
}

#[derive(Properties, PartialEq)]
pub struct RustCodeEditorProps {
    pub value: AttrValue,
    pub on_change: Callback<String>,
    #[prop_or_default]
    pub placeholder: AttrValue,
    /// Extra classes on the editor wrapper. When empty, the editor is `h-40`;
    /// pass a size (e.g. `flex-1 min-h-0`) to make it fill its container.
    #[prop_or_default]
    pub class: Classes,
}

/// An editable code body with syntax highlighting.
#[function_component(RustCodeEditor)]
pub fn rust_code_editor(props: &RustCodeEditorProps) -> Html {
    let backdrop = use_node_ref();
    let textarea = use_node_ref();

    // Keep the highlighted backdrop scrolled in step with the textarea.
    {
        let backdrop = backdrop.clone();
        let textarea = textarea.clone();
        use_effect_with_deps(
            move |_| {
                sync_scroll(&textarea, &backdrop);
                || ()
            },
            props.value.clone(),
        );
    }

    let on_input = {
        let on_change = props.on_change.clone();
        Callback::from(move |event: InputEvent| {
            let textarea = event.target_unchecked_into::<HtmlTextAreaElement>();
            on_change.emit(textarea.value());
        })
    };

    let on_scroll = {
        let backdrop = backdrop.clone();
        let textarea = textarea.clone();
        Callback::from(move |_: Event| sync_scroll(&textarea, &backdrop))
    };

    let size = if props.class.is_empty() {
        classes!("h-40")
    } else {
        props.class.clone()
    };

    html! {
        <div class={classes!("code-editor", "relative", "rounded", "border", "border-line", "font-mono", "text-xs", "leading-5", size)}>
            <pre ref={backdrop} class="code-highlight">{ highlighted(&props.value) }</pre>
            <textarea
                ref={textarea}
                class="code-input"
                spellcheck="false"
                placeholder={props.placeholder.clone()}
                value={props.value.clone()}
                oninput={on_input}
                onscroll={on_scroll}
            />
        </div>
    }
}

fn sync_scroll(textarea: &NodeRef, backdrop: &NodeRef) {
    if let (Some(textarea), Some(backdrop)) = (
        textarea.cast::<HtmlTextAreaElement>(),
        backdrop.cast::<Element>(),
    ) {
        backdrop.set_scroll_top(textarea.scroll_top());
        backdrop.set_scroll_left(textarea.scroll_left());
    }
}
