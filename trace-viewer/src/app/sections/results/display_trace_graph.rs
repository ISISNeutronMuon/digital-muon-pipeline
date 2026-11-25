use crate::{
    app::{components::DisplayErrors, sections::results::context::ResultsLevelContext},
    structs::TracePlotly,
};
use leptos::{IntoView, component, prelude::*, view};

/// Panel which renders a `TracePlotly` if one is selected.
#[component]
pub(crate) fn DisplayTrace() -> impl IntoView {
    let create_and_fetch_plotly = use_context::<ResultsLevelContext>()
        .expect("ResultsLevelContext should be provided, this should never fail")
        .create_and_fetch_plotly;

    view! {
        <Transition fallback = ||view!("Loading Graph")>
            {move ||create_and_fetch_plotly.value().get().map(|trace| view!{
                <ErrorBoundary fallback = |errors| view!{ <DisplayErrors errors /> }>
                    {trace.map(|trace_plotly|
                        view!{ <DisplayGraph trace_plotly /> }
                    )}
                </ErrorBoundary>
            })}
        </Transition>
    }
}

/// Displays a `TracePlotly` object.
///
/// # Parameters
/// - trace_plotly: the trace and eventlists to render.
#[component]
pub(crate) fn DisplayGraph(trace_plotly: TracePlotly) -> impl IntoView {
    let TracePlotly {
        title,
        json,
        file,
        csv
    } = trace_plotly;

    view! {
        <div class = "content trace-graph" id = "trace-graph">
            <div class = "trace-graph-title">
                {title.clone()}
            </div>
            <div class = "trace-graph-button">
                <div class = "trace-graph-buttons">
                    <a href = {format!("data:text/html;charset=utf-8,{}", csv)} download=format!("Trace of {title}.csv")>
                        Download CSV
                    </a>
                </div>
                <div class = "trace-graph-button">
                    <a href = {format!("data:text/html;charset=utf-8,{}", file)} download=format!("Trace of {title}.html")>
                        Download HTML
                    </a>
                </div>
            </div>
            <div>
                <div id="trace-graph" class="plotly-graph-div"></div>
                <script type="text/javascript" inner_html = {
                    format!("Plotly.newPlot('trace-graph', {});", json)
                }>
                </script>
            </div>
        </div>
    }
}
