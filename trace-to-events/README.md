# trace-to-events

## Introduction

This tool converts traces to events using a composable pipeline of signal modifiers and event detectors.
Traces are consumed from a kafka broker, processed into events and the resulting event list messages is sent to the same broker.

## Command Line Interface

`trace-to-events [OPTIONS] --broker <BROKER> [COMMAND]`

For instance:

```shell
trace-to-events --broker localhost:19092 --trace-topic Traces --event-topic Events --polarity <POLARITY> --group trace-to-events
```

The trace topic is the kafka topic that trace messages are consumed from, and event topic is the topic that event messages are produced to.
Polarity is the direction (positive or negative) in which the trace signal responds to events.

For instructions run:

```shell
trace-to-events --help
```

### Commands

- `fixed-threshold-discriminator`: Detects events using a fixed threshold discriminator. Events consist only of a time value.
- `advanced-muon-detector`: Detects events using differential discriminators. Event lists consist of time and voltage values.
- `help`: Print this message or the help of the given subcommand(s)

### Fixed Threshold Discriminator

`trace-to-events --broker <BROKER> fixed-threshold-discriminator --threshold <THRESHOLD>`

```shell
      --threshold <THRESHOLD>  If the detector is armed, an event is registered when the trace passes this value for the given duration
      --duration <DURATION>    The duration, in samples, that the trace must exceed the threshold for [default: 1]
      --cool-off <COOL_OFF>    After an event is registered, the detector disarms for this many samples [default: 0]
```

Threshold is the real threshold value, duration is how long the signal should be beyond the threshold to trigger an event (should be positive), and cool_down is how long before another detection can be found (should be non-negative).

### Advanced Muon Detector

`trace-to-events --broker <BROKER> advanced-muon-detector [OPTIONS] --baseline-length <BASELINE_LENGTH> --smoothing-window-size <SMOOTHING_WINDOW_SIZE> --muon-onset <MUON_ONSET> --muon-fall <MUON_FALL> --muon-termination <MUON_TERMINATION>`

```shell
      --baseline-length <BASELINE_LENGTH>
          Size of initial portion of the trace to use for determining the baseline. Initial portion should be event free.
      --smoothing-window-size <SMOOTHING_WINDOW_SIZE>
          Size of the moving average window to use for the lopass filter.
      --muon-onset <MUON_ONSET>
          Differential threshold for detecting muon onset (threshold,duration,cool_down). See README.md.
      --muon-fall <MUON_FALL>
          Differential threshold for detecting muon peak (threshold,duration,cool_down). See README.md.
      --muon-termination <MUON_TERMINATION>
          Differential threshold for detecting muon termination (threshold,duration,cool_down). See README.md.
      --max-amplitude <MAX_AMPLITUDE>
          Optional parameter which (if set) filters out events whose peak is greater than the given value.
      --min-amplitude <MIN_AMPLITUDE>
          Optional parameter which (if set) filters out events whose peak is less than the given value.
  -h, --help
          Print help
```

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
