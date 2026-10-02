mod components;
mod main_content;
mod sections;
mod server_functions;
mod topbar;

use crate::structs::ClientSideData;
use cfg_if::cfg_if;
use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};
use main_content::Main;
use topbar::TopBar;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        pub(crate) use server_functions::{ServerError, SessionError};
    }
}

/// This struct enable a degree of type-checking for the [use_context]/[use_context] functions.
/// Any component making use of the following fields should call `use_context::<TopLevelContext>()`
/// and select the desired field.
#[derive(Clone)]
pub(crate) struct TopLevelContext {
    pub(crate) client_side_data: ClientSideData,
}

/// Creates outermost html tags, the header and body containing the [App] component.
/// 
/// # Parameters
/// - leptos_options: options utilised in some of Leptos's helper tags.
pub fn shell(leptos_options: LeptosOptions) -> impl IntoView + 'static {
    let public_url = use_context::<ClientSideData>()
        .expect("ClientSideData should be provided, this should never fail.")
        .public_url;

    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <script src="https://cdn.plot.ly/plotly-2.14.0.min.js"></script>
                <AutoReload options=leptos_options.clone() />
                <HydrationScripts options=leptos_options.clone() root = public_url.clone() />
                <HashedStylesheet options=leptos_options root = public_url />
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

/// An app router which renders the homepage and handles 404's
#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    let client_side_data = SharedValue::new(|| {
        use_context::<ClientSideData>()
            .expect("TopLevelContext should be provided, this should never fail.")
    })
    .into_inner();
    #[cfg(feature = "hydrate")]
    let public_path = client_side_data.public_url.path().to_string();
    provide_context(TopLevelContext { client_side_data });

    view! {
        // sets the document title
        <Title text="Trace Viewer" />

        // injects metadata in the <head> of the page
        <Meta charset="UTF-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1.0" />

        <TopBar />
        <Router base=cfg_if! { if #[cfg(feature = "hydrate")] { public_path } else { "" } }>
            <Routes fallback = NotFound>
                <Route path = path!("") view = Main />
            </Routes>
        </Router>
    }
}

/// To display if the required page is not found.
#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <h1>"Trace Viewer"</h1>
        <p> "Page Not Found" </p>
    }
}
