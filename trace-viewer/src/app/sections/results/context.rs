use crate::app::server_functions::{CreateAndFetchPlotly, CreateAndDownloadPlotly};
use leptos::prelude::*;

/// This struct enable a degree of type-checking for the [use_context]/[use_context] functions.
/// Any component making use of the following fields should call `use_context::<ResultsLevelContext>()`
/// and select the desired field.
#[derive(Clone)]
pub(super) struct ResultsLevelContext {
    /// Server action creating a plotly image when from a given session, stored trace index, and channel.
    pub(super) create_and_fetch_plotly: ServerAction<CreateAndFetchPlotly>,
    /// Server action creating a plotly image when from a given session, stored trace index, and channel.
    pub(super) create_and_download_plotly: ServerAction<CreateAndDownloadPlotly>,
    /// Specifies whether only the selected channels, or all channels in a message, should be shown.
    pub(super) selected_channels_only: RwSignal<bool>,
}
