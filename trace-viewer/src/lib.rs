#![recursion_limit = "256"]

pub mod app;
pub mod structs;

pub use app::{App, shell};
use cfg_if::cfg_if;
use chrono::{DateTime, Utc};

/// Used by instances of the website to refer to server-side sessions.
pub type Uuid = Option<String>;
/// The timestamp type with timezone.
pub type Timestamp = DateTime<Utc>;

/*
    The following types are redefined manually rather than being imported
    from [common] as that module cannot be included in the wasm target.
    Maybe this can be fixed later?
*/
pub type Channel = u32;
pub type Time = u32;
pub type Intensity = u16;
pub type DigitizerId = u8;
pub type FrameNumber = u32;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        pub mod sessions;
        pub mod finder;
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::TopLevelContext;
    use leptos::prelude::use_context;

    console_error_panic_hook::set_once();

    leptos::mount::hydrate_body(App);

    let client_side_data = use_context::<TopLevelContext>()
        .expect("TopLevelContext should exists, this should never fail.")
        .client_side_data;

    // The `leak` consumes the `String`, marks it's heap allocation as `'static`
    // and returns a static reference to it.
    // This only results in an actual memory leak if the returned reference is ever dropped.
    // By passing it to `set_server_url` we ensure this doesn't happen until the app is closed.
    // Maybe, one day, leptos will allow `set_server_url` to be a String, allowing us to avoid
    // having to use this scary sounding `leak` method... but this is not that day.
    let public_url: &'static str = client_side_data.public_url.to_string().leak();
    leptos::server_fn::client::set_server_url(public_url);
}
