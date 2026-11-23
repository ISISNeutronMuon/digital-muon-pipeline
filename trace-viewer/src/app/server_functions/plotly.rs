use crate::structs::{SelectedTraceIndex, TracePlotly};
use cfg_if::cfg_if;
use leptos::prelude::*;
use tracing::instrument;

/// From a session specified by the uuid, creates a `TracePlotly` object
/// from the given index and channel of the session.
///
/// # Parameters
/// - uuid: the uuid of the session.
/// - index_and_channel: specifies the index of the session's stored trace message,
/// and the channel of that message to create the `TracePlotly` object.
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

    let plot = create_plotly(metadata, index_and_channel.channel, trace, eventlists)?;
    
    Ok(TracePlotly {
        title: format!("Channel {} from Digitiser {}", index_and_channel.channel, metadata.id),
        json: plot.to_json(),
        index_and_channel
        //eventlist_data: eventlists.map(|eventlist|eventlist.to_json()).collect(),
    })
}

/// From a session specified by the uuid, creates a `TracePlotly` object
/// from the given index and channel of the session.
///
/// # Parameters
/// - uuid: the uuid of the session.
/// - index_and_channel: specifies the index of the session's stored trace message,
/// and the channel of that message to create the `TracePlotly` object.
#[server]
#[instrument(skip_all, err(level = "warn"))]
pub async fn create_and_download_plotly(
    uuid: String,
    index_and_channel: SelectedTraceIndex,
) -> Result<String, ServerFnError> {
    use actix_web::http::header::{CONTENT_TYPE, CONTENT_DISPOSITION, HeaderValue};
    use leptos_actix::ResponseOptions;

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

    let plot = create_plotly(metadata, index_and_channel.channel, trace, eventlists)?;
    let opts = expect_context::<ResponseOptions>();
    let content_disposition_value = HeaderValue::from_str("attachment; filename=\"trace.html\"")
        .expect("This should parse, this should never fail");
    opts.insert_header(CONTENT_DISPOSITION, content_disposition_value);
    let content_type_value = HeaderValue::from_str("text/html")
        .expect("This should parse, this should never fail");
    opts.insert_header(CONTENT_TYPE, content_type_value);
    Ok(plot.to_html())
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use crate::{
            app::SessionError,
            structs::{DigitiserMetadata, Trace as MuonTrace, EventList, ServerSideData},
            Channel
        };
        use plotly::{
            Layout, Scatter, Plot,
            color::NamedColor,
            common::{Line, Marker, MarkerSymbol, Mode},
            layout::{Axis, ModeBar},
        };
        use tracing::info;
        const COLOURS: [NamedColor; 6] = [NamedColor::IndianRed, NamedColor::DarkGreen, NamedColor::Indigo, NamedColor::MediumSpringGreen, NamedColor::HotPink, NamedColor::YellowGreen];
        const MARKERS: [MarkerSymbol; 5] = [MarkerSymbol::CircleOpen, MarkerSymbol::SquareOpen, MarkerSymbol::Cross, MarkerSymbol::DiamondOpen, MarkerSymbol::X];

        /// Creates a `TracePlotly` object from the given parameters.
        ///
        /// # Parameters
        /// - metadata: metadata from the trace's parent digitiser message.
        /// - channel: the channel number of the trace.
        /// - trace: the raw trace.
        /// - eventlists: list of eventlists from the selected topics, tagged with the topic names.
        fn create_plotly<'a>(metadata: &DigitiserMetadata, channel: Channel, trace: &'a MuonTrace, eventlists: Vec<(String, &'a EventList)>) -> Result<Plot, ServerFnError> {
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
                .marker(Marker::new().color(*colour).symbol(symbol.clone()).opacity(0.75))
                .name(format!{"Events: {event_topic}"})
            );

            let mut plot = Plot::new();
            plot.add_trace(trace);
            for eventlist in eventlists {
                plot.add_trace(eventlist);
            }
            plot.set_layout(layout);
            Ok(plot)
        }
    }
}
