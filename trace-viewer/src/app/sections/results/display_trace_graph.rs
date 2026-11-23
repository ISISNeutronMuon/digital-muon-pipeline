use crate::{
    app::{components::DisplayErrors, main_content::MainLevelContext, sections::results::context::ResultsLevelContext, server_functions::CreateAndDownloadPlotly},
    structs::TracePlotly,
};
use leptos::{IntoView, component, ev::MouseEvent, prelude::*, view};
use leptos_router::{NavigateOptions, hooks::use_navigate};

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
        index_and_channel,
    } = trace_plotly;
/*
    let data = if eventlist_data.is_empty() {
        trace_data
    } else {
        format!("{trace_data}, {}", eventlist_data.join(","))
    };
 */

    /*const PLOTLY_JS_SCRIPT: &str = "https://cdn.plot.ly/plotly-3.7.0.min.js";
    let html = HtmlPage::new()
        .with_title(&title)
        .with_script_link(PLOTLY_JS_SCRIPT)
        .with_header(1, &title)
        .with_raw(html.clone());*/
    
    let create_and_download_plotly = use_context::<ResultsLevelContext>()
        .expect("ResultsLevelContext should be provided, this should never fail")
        .create_and_download_plotly;

    let uuid = use_context::<MainLevelContext>()
        .expect("MainLevelContext should be provided, this should never fail.")
        .uuid;

    let on_click = {
        let this_index_and_channel = index_and_channel.clone();
        move |_: MouseEvent| {
            if let Some(uuid) = uuid.get() {
                create_and_download_plotly.dispatch(CreateAndDownloadPlotly {
                    uuid,
                    index_and_channel: this_index_and_channel.clone(),
                });
            }
        }
    };

    Effect::new(move ||if let Some(Ok(data)) = create_and_download_plotly.value().get() {
        use_navigate()(&format!("data:text/html;charset=utf-8,{data}"), NavigateOptions {
            resolve: true,
            replace: false,
            scroll: false,
            state: Default::default()
        });
    });

    view! {
        <div class = "content trace-graph" id = "trace-graph">
            <div class = "trace-graph-title">
                {title}
            </div>
            <div id="trace-graph" class="plotly-graph-div"></div>
            <script type="text/javascript" inner_html = {
                format!("Plotly.newPlot('trace-graph', {});", json)
            }>
            </script>
            <div>
                /*<div>
                    <a href = {format!("data:text/csv;charset=utf-8,{data}")}>
                        Download CSV
                    </a>
                </div>*/
                <a href = "" on:click = on_click download>
                    Download HTML
                    /*<a href = {format!("data:text/html;charset=utf-8,{}", html.to_html_string())}>
                    </a>*/
                </a>
            </div>
        </div>
    }
}
