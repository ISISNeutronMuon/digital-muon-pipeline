use crate::structs::{SelectedTraceIndex, TracePlotly};
use cfg_if::cfg_if;
use leptos::prelude::*;
use tracing::instrument;

#[server]
#[instrument(skip_all, err(level = "warn"))]
pub async fn create_and_fetch_plotly(
    uuid: String,
    index_and_channel: SelectedTraceIndex,
) -> Result<TracePlotly, ServerFnError> {
    let session_engine_arc_mutex = use_context::<ServerSideData>()
        .expect("ServerSideData should be provided, this should never fail.")
        .session_engine;

    let session_engine = session_engine_arc_mutex.lock().await;

    let (metadata, digitiser_traces) = session_engine
        .session(&uuid)?
        .get_selected_trace(index_and_channel.index)?;

    let trace = digitiser_traces
        .traces
        .get(&index_and_channel.channel)
        .ok_or(SessionError::ChannelNotFound)?;

    let eventlists = digitiser_traces.events
        .iter()
        .flat_map(|(&topic_idx, events)|events.get(&index_and_channel.channel).map(|events|(topic_idx, events)))
        .collect::<Vec<_>>();

    create_plotly(metadata, index_and_channel.channel, trace, eventlists)
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use crate::{
            app::SessionError,
            structs::{DigitiserMetadata, Trace as MuonTrace, EventList, ServerSideData},
            Channel
        };
        use plotly::{
            Layout, Scatter, Trace,
            color::NamedColor,
            common::Mode,
            common::{Line, Marker},
            layout::{Axis, ModeBar},
        };
        use tracing::info;

        fn create_plotly<'a>(metadata: &DigitiserMetadata, channel: Channel, trace: &'a MuonTrace, eventlists: Vec<(usize, &'a EventList)>) -> Result<TracePlotly, ServerFnError> {
            info!("create_plotly_on_server");

            let date = metadata.timestamp.date_naive().to_string();
            let time = metadata.timestamp.time().to_string();
            let layout = Layout::new()
                .title(format!("Channel {channel}, digitiser {}, in frame {} at<br>{time} on {date}.", metadata.id, metadata.frame_number))
                .mode_bar(ModeBar::new().background_color(NamedColor::LightGrey))
                .show_legend(true)
                .auto_size(true)
                .x_axis(Axis::new().title("Time (ns)"))
                .y_axis(Axis::new().title("Intensity"));

            let trace = Scatter::new(
                (0..trace.len()).collect::<Vec<_>>(),
                trace.clone(),
            )
            .mode(Mode::Lines)
            .name("Trace")
            .line(Line::new().color(NamedColor::CadetBlue));

            let eventlists = eventlists.into_iter().map(|(topic_idx, eventlist)| {
                Scatter::new(
                    eventlist.iter().map(|event| event.time).collect::<Vec<_>>(),
                    eventlist
                        .iter()
                        .map(|event| event.intensity)
                        .collect::<Vec<_>>(),
                )
                .mode(Mode::Markers)
                .name(&format!{"Events {topic_idx}"})
                .marker(Marker::new().color(NamedColor::IndianRed))
            });

            Ok(TracePlotly {
                title: format!("Channel {} from Digitiser {}", channel, metadata.id),
                trace_data: trace.to_json(),
                eventlist_data: eventlists.map(|eventlist|eventlist.to_json()).collect(),
                layout: layout.to_json(),
            })
        }
    }
}
