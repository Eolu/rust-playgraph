//! JSON import/export of a playground.

use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use web_sys::{Blob, File, FileReader, HtmlAnchorElement, HtmlInputElement, Url};

use playgraph_core::Playground;
use playgraph_core::model::{Edge, GraphNode, StageDef};
use serde::{Deserialize, Serialize};

/// What we keep in the browser between visits: the user's own stage
/// definitions, the composed graph, and the outer-scope code block. Built-in
/// library stages are not stored (they ship with the app); nodes reference them
/// by name.
#[derive(Default, Serialize, Deserialize)]
pub struct StoredDoc {
    #[serde(default)]
    pub stages: Vec<StageDef>,
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub prelude: String,
}

const STORAGE_KEY: &str = "rust-playgraph.doc.v1";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// Load the saved document, if any.
pub fn load_doc() -> Option<StoredDoc> {
    let raw = storage()?.get_item(STORAGE_KEY).ok()??;
    serde_json::from_str(&raw).ok()
}

/// Persist the document. Failures (e.g. quota) are ignored.
pub fn save_doc(doc: &StoredDoc) {
    if let Some(storage) = storage()
        && let Ok(json) = serde_json::to_string(doc)
    {
        let _ = storage.set_item(STORAGE_KEY, &json);
    }
}

/// Trigger a browser download of the playground as pretty JSON.
pub fn export(playground: &Playground, filename: &str) {
    let Ok(json) = serde_json::to_string_pretty(playground) else {
        return;
    };
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(&json));
    let Ok(blob) = Blob::new_with_str_sequence(&parts) else {
        return;
    };
    let Ok(url) = Url::create_object_url_with_blob(&blob) else {
        return;
    };
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Ok(anchor) = document
        .create_element("a")
        .and_then(|element| element.dyn_into::<HtmlAnchorElement>().map_err(Into::into))
    else {
        return;
    };
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    let _ = Url::revoke_object_url(&url);
}

/// Read a picked file as text and hand it to `on_text`.
pub fn import(file: File, on_text: impl Fn(String) + 'static) {
    let Ok(reader) = FileReader::new() else {
        return;
    };
    let result_reader = reader.clone();
    let onload = Closure::<dyn FnMut()>::new(move || {
        if let Ok(result) = result_reader.result()
            && let Some(text) = result.as_string()
        {
            on_text(text);
        }
    });
    reader.set_onload(Some(onload.as_ref().unchecked_ref()));
    onload.forget();
    let _ = reader.read_as_text(&file);
}

/// First file from a file input, if any.
pub fn first_file(input: &HtmlInputElement) -> Option<File> {
    input.files().and_then(|files| files.get(0))
}
