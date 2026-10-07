//! Shows the output of the last run.

use yew::prelude::*;

use crate::components::rust_code::RustCodeBlock;
use crate::context::use_run;

#[function_component(OutputPanel)]
pub fn output_panel() -> Html {
    let run = use_run();

    html! {
        <div class="p-4 font-mono text-xs">
            if let Some(error) = &run.transport_error {
                <div class="text-danger mb-2">
                    { format!("could not reach the backend: {error}") }
                    <div class="text-ink-dim mt-1">
                        {"start it with `cargo run -p playgraph-backend` (default 127.0.0.1:3001)"}
                    </div>
                </div>
            }

            {
                match &run.result {
                    None => html! {
                        <div class="text-ink-dim">{"Run the graph to compile and execute it."}</div>
                    },
                    Some(result) => html! {
                        <div class="space-y-2">
                            <div class="flex gap-4 text-ink-dim">
                                <span>{ format!("exit: {:?}", result.exit_code) }</span>
                                <span>{ format!("{} ms", result.duration_ms) }</span>
                                <span>{ if result.ok { "ok" } else { "failed" } }</span>
                            </div>

                            if let Some(error) = &result.error {
                                <pre class="text-danger whitespace-pre-wrap">{ error }</pre>
                            }
                            if !result.stdout.is_empty() {
                                <pre class="whitespace-pre-wrap text-ink-bright">{ &result.stdout }</pre>
                            }
                            if !result.stderr.is_empty() {
                                <pre class="text-warning whitespace-pre-wrap">{ &result.stderr }</pre>
                            }
                            if let Some(generated) = &result.generated {
                                <details class="text-ink-dim">
                                    <summary class="cursor-pointer">{"generated code"}</summary>
                                    <RustCodeBlock code={generated.clone()} class={classes!("mt-2", "text-ink")} />
                                </details>
                            }
                        </div>
                    },
                }
            }
        </div>
    }
}
