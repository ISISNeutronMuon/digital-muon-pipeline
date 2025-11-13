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

    let eventlists = digitiser_traces
        .events
        .iter()
        .flat_map(|(&topic_idx, events)| {
            events.get(&index_and_channel.channel).map(|events| {
                (
                    session_engine
                        .settings()
                        .topics
                        .digitiser_event_topic
                        .get(topic_idx)
                        .expect("Daq eventlist topic index should exist, this should never fail.")
                        .clone(),
                    events,
                )
            })
        })
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
            common::{Line, Marker, MarkerSymbol, Mode},
            layout::{Axis, ModeBar},
        };
        use tracing::info;
        const COLOURS: [NamedColor; 6] = [NamedColor::IndianRed, NamedColor::DarkGreen, NamedColor::Indigo, NamedColor::MediumSpringGreen, NamedColor::HotPink, NamedColor::YellowGreen];
        const MARKERS: [MarkerSymbol; 5] = [MarkerSymbol::CircleOpen, MarkerSymbol::SquareOpen, MarkerSymbol::Cross, MarkerSymbol::DiamondOpen, MarkerSymbol::X];

        fn create_plotly<'a>(metadata: &DigitiserMetadata, channel: Channel, trace: &'a MuonTrace, eventlists: Vec<(String, &'a EventList)>) -> Result<TracePlotly, ServerFnError> {
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

            let eventlists = eventlists.into_iter()
                .zip(COLOURS.iter().cycle().zip(MARKERS.iter().cycle()))
                .map(|((event_topic, eventlist), (colour, symbol))|
                Scatter::new(
                    eventlist.iter().map(|event| event.time).collect::<Vec<_>>(),
                    eventlist
                        .iter()
                        .map(|event| event.intensity)
                        .collect::<Vec<_>>(),
                )
                .mode(Mode::Markers)
                .marker(Marker::new().color(*colour).symbol(symbol.clone()).opacity(0.5))
                .name(format!{"Events: {event_topic}"})
            );

            Ok(TracePlotly {
                title: format!("Channel {} from Digitiser {}", channel, metadata.id),
                trace_data: trace.to_json(),
                eventlist_data: eventlists.map(|eventlist|eventlist.to_json()).collect(),
                layout: layout.to_json(),
            })
        }
    }
}
