# trace-to-events

## Introduction

This tool converts traces to events using a composable pipeline of signal modifiers and event detectors.
Traces are consumed from a kafka broker, processed into events and the resulting event list messages is sent to the same broker.

## Command Line Interface

`trace-to-events [OPTIONS] --broker <BROKER> --consumer-group <CONSUMER_GROUP> --trace-topic <TRACE_TOPIC> --event-topic <EVENT_TOPIC> --config-topic <CONFIG_TOPIC>`

For instance:

```shell
trace-to-events --broker localhost:19092 --consumer-group trace-to-events --trace-topic Traces --event-topic Events --config-topic Config
```

- `--trace-topic`: The Kafka topic from which trace messages are consumed.
- `--event-topic`: The Kafka topic to which digitiser event list messages are produced.
- `--config-topic`: The Kafka topic from which event formation configuration JSON documents are consumed.

For instructions run:

```shell
trace-to-events --help
```

## Dynamic Event Formation Configuration

Event formation parameters are configured by publishing a JSON document to the Kafka topic specified by `--config-topic`.

- **On boot:** `trace-to-events` inspects `--config-topic`, retrieves the last/most recent message, and applies that configuration.
- **Dynamic Reconfiguration:** When a new configuration message is published to `--config-topic`, the event formation pipeline is dynamically reconfigured and the new settings are applied to subsequent trace messages.
- **Unconfigured State:** If no configuration is currently applied (e.g. before any configuration has been published to `--config-topic`), `trace-to-events` discards incoming trace messages until a valid configuration is received.

### Configuration Schema

The JSON document schema corresponds directly to the structs and enums defined in [`src/parameters.rs`](src/parameters.rs):

- [`DetectorSettings`](src/parameters.rs): Top-level configuration document containing `mode`, `polarity`, and optional `baseline`.
- [`Polarity`](src/parameters.rs): Signal polarity (`"positive"`, `"negative"`).
- [`Mode`](src/parameters.rs): Selected detector algorithm and its parameters:
   - `FixedThresholdDiscriminator`: Configured by [`FixedThresholdDiscriminatorParameters`](src/parameters.rs) (see [`examples/fixed-threshold-discriminator.json`](examples/fixed-threshold-discriminator.json)).
   - `DifferentialThresholdDiscriminator`: Configured by [`DifferentialThresholdDiscriminatorParameters`](src/parameters.rs) along with [`PeakHeightMode`](src/parameters.rs) and [`PeakHeightBasis`](src/parameters.rs) (see [`examples/differential-threshold-discriminator.json`](examples/differential-threshold-discriminator.json)).
   - `SmoothingDetector`: Configured by [`SmoothingDetectorParameters`](src/parameters.rs) (see [`examples/smoothing-detector.json`](examples/smoothing-detector.json)).
   - `Multiscaling`: Configured by [`MultiscalingDetectorParameters`](src/parameters.rs) and [`MultiscalingDetectorMethod`](src/parameters.rs) (see [`examples/multiscaling.json`](examples/multiscaling.json)).

Field names and enum variants map to `kebab-case`.

## Configuring the Detector Pipeline

Given an iterator of type u16 (aliased as Intensity in the crate), the pipeline is setup as follows:

```rust
// trace : Vec<u16>
let raw = trace
    .into_iter()
    .enumerate()
    .map(|(i, v)| (i as Real, v as Real));
```

Then the trace signal can be processed using window functions (e.g. smoothing, finite differences). For instance:

```rust
let smoothed = raw
    .window(Baseline::new(100, 0.1))
    .window(SmoothingWindow::new(5))    // this produces values of type Stats { value : Real, mean : Real, variance : Real }
    .map(|(i, stats)| (i, stats.mean));
```

The signal is first baselined (the baseline is estimated from an exponential average of the first 100 values, and then subtracted off),
then a moving average window of size 5 is applied to the signal, finally the mean of the signal is extracted.

Next the signal is transformed into events:

```rust
let events = smoothed
    .window(FiniteDifferences::<2>::new())  // this produces size 2 arrays: [trace value, 1st-difference of trace]
    .events(AdvancedMuonDetector::new(
        advanced_parameters.muon_onset,
        advanced_parameters.muon_fall,
        advanced_parameters.muon_termination,
        advanced_parameters.duration
    ))
```

and finally the events are assembled into pulses.

```rust
let pulses = events.assemble(BasicMuonAssembler::default())
    .collect();
```

## Window Functions

- `Baseline`: this estimates the baseline of the signal from the easliest occuring samples. Once this is found the remaining signal has the baseline subtracted. Note that this requires the initial samples to be event free.
- `FiniteDifferences<N>`: this reads in `N` samples and outputs a `RealArray` of the first `N` finite differences.
- `SmoothingWindow`: this reads in a user-specified number of samples and outputs a `Stats` object calculated from the moving-average window. Each subsequent input updates the moving-average window and outputs the resulting `Stats` object.

## Detectors

- Advanced Muon Detector
- Fixed Threshold Detector

## Data Types

- Real: an alias for `f64`
- Intensity: an alias for `u16`
- Time: an alias for `u32`
- TraceArray: a wrapper for an array of TraceValue types.
- RealArray: a specification of TraceArray to Real values.
- Pulse: a datatype which represents all properties a detector may record. All fields are optional.

## Traits

- Temporal: abstracts all possible types representing time, mainly (Real or Time)
- TraceValue: abstracts over all possible types representing voltage (mainly Real or RealArray).
- TracePoint: abstracts types representing points in the trace signal. This is mainly just a tuple `(T,V)`, where `T : Temporal` and `V : TraceValue`.

- EventData: abstracts all possible types representing event data. Each detector type has its own EventData type.
- EventPoint: abstracts types representing points in the trace signal. This is mainly just a tuple `(T,E)`, where `T : Temporal` and `E : EventData`.
