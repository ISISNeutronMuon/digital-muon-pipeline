use crate::{analysis::AnalysisEngine, eventlists::EventlistsCollection};
use std::{io, time::Duration};
use tokio::{
    select,
    signal::unix::{Signal, SignalKind, signal},
    sync::mpsc::{Receiver, Sender, channel},
    task::JoinHandle,
};
use tracing::{error, info, trace, warn};

// The following functions control the eventlist evaluator thread.
/// Create a new thread and setup the evaluator task.
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [EventlistsCollection::span()] into the Kafka header.
/// - send_frame_buffer_size: the maximum number of [EventlistsCollection] objects to store in the channel's buffer. If the buffer is filled, then sending another will block until there is sufficient space in the buffer.
/// - analysis_engine: the analysis engine object.
pub(crate) fn create_evaluator_task(
    use_otel: bool,
    analysis_engine: AnalysisEngine,
    send_frame_buffer_size: usize,
) -> io::Result<(Sender<EventlistsCollection>, JoinHandle<()>)> {
    let (channel_send, channel_recv) = channel::<EventlistsCollection>(send_frame_buffer_size);

    let sigint = signal(SignalKind::interrupt())?;
    let handle = tokio::spawn(recv_and_evaluate(
        use_otel,
        analysis_engine,
        channel_recv,
        sigint,
    ));
    Ok((channel_send, handle))
}

/// Runs infinitely, and waits on any [EventlistsCollection]s received through the given receive channel.
///
/// Calling this function returns a Future, which should be passed to a async task,
/// as in function [create_evaluator_task]. The general form of this is:
/// ```rust
/// let join_handle = tokio::spawn(create_evaluator_task(...))?;
/// ```
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [EventlistsCollection::span()] into the Kafka header.
/// - channel_recv: receive channel that can receive [EventlistsCollection] objects.
/// - analysis_engine: the analysis engine object.
/// - sigint: FIXME.
async fn recv_and_evaluate(
    use_otel: bool,
    mut analysis_engine: AnalysisEngine,
    mut channel_recv: Receiver<EventlistsCollection>,
    mut sigint: Signal,
) {
    let mut chart_poll_interval = tokio::time::interval(Duration::from_millis(2000));
    loop {
        select! {
            message = channel_recv.recv() => {
                // Blocks until a frame is received
                match message {
                    Some(eventlists_collection) => {
                        evaluate_eventlists_collection(use_otel, &mut analysis_engine, eventlists_collection).await;
                    }
                    None => {
                        info!("Send-Frame channel closed");
                        return;
                    }
                }
            }
            _ = chart_poll_interval.tick() => {
                trace!("Polling for chart completion.");
                if analysis_engine.chart_poll().expect("This should never fail.") {
                    match analysis_engine.build_charts() {
                        Ok(_) => (),
                        Err(e) => {
                            error!("{e}");
                        },
                    }
                    close_and_flush_evaluate_channel(use_otel, &mut analysis_engine ,&mut channel_recv).await;
                };
            }
            _ = sigint.recv() => {
                close_and_flush_evaluate_channel(use_otel, &mut analysis_engine ,&mut channel_recv).await;
            }
        }
    }
}

/// Closes the evaluator channel and dispatch all [EventlistsCollection]s remaining in the channel.
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [EventlistsCollection::span()] into the Kafka header.
/// - channel_recv: receive channel that can receive [EventlistsCollection] objects.
/// - analysis_engine: the analysis engine object.
#[tracing::instrument(skip_all, name = "Closing", level = "info", fields(capacity = channel_recv.capacity(), max_capacity = channel_recv.max_capacity()))]
async fn close_and_flush_evaluate_channel(
    use_otel: bool,
    analysis_engine: &mut AnalysisEngine,
    channel_recv: &mut Receiver<EventlistsCollection>,
) -> Option<()> {
    channel_recv.close();

    loop {
        let eventlists_collection = channel_recv.recv().await?;
        flush_eventlists_collection(use_otel, analysis_engine, eventlists_collection).await?;
    }
}

/// Dispatches the given frame to the Kafka broker on the given topic.
///
/// This function exists just to encapsulate [evaluate_eventlists_collection] in a span, it might be better to do this directly in [close_and_flush_evaluate_channel].
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [EventlistsCollection::span()] into the Kafka header.
/// - eventlists_collection: the [EventlistsCollection] to dispatch.
/// - analysis_engine: the analysis engine object.
#[tracing::instrument(skip_all, name = "Flush Frame")]
async fn flush_eventlists_collection(
    use_otel: bool,
    analysis_engine: &mut AnalysisEngine,
    eventlists_collection: EventlistsCollection,
) -> Option<()> {
    evaluate_eventlists_collection(use_otel, analysis_engine, eventlists_collection).await;
    Some(())
}

/// Dispatches the given frame to the Kafka broker on the given topic.
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [EventlistsCollection::span()] into the Kafka header.
/// - eventlists_collection: the [EventlistsCollection] to dispatch.
/// - analysis_engine: the analysis engine object.
#[tracing::instrument(skip_all)]
async fn evaluate_eventlists_collection(
    use_otel: bool,
    analysis_engine: &mut AnalysisEngine,
    eventlists_collection: EventlistsCollection,
) {
    match analysis_engine.push(eventlists_collection) {
        Ok(_) => (),
        Err(e) => {
            warn!("Error {e}")
        }
    }
}
