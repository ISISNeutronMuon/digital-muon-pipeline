use super::otel_tracer::{OtelOptions, OtelTracer};
use opentelemetry_otlp::ExporterBuildError;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::{Span, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt};

pub struct TracerOptions<'a> {
    otel_options: Option<OtelOptions<'a>>,
}

impl<'a> TracerOptions<'a> {
    pub fn new(endpoint: Option<&'a str>, namespace: String) -> Self {
        Self {
            otel_options: endpoint.map(|endpoint| OtelOptions {
                endpoint,
                namespace,
            }),
        }
    }
}

/// This object initialises all tracers, given a TracerOptions struct.
/// If TracerOptions contains a OtelOptions struct then it initialises the
/// OtelTracer object as well.
pub struct TracerEngine {
    use_otel: bool,
    otel_tracer_provider: Option<SdkTracerProvider>,
    otel_setup_error: Option<ExporterBuildError>,
}

impl TracerEngine {
    /// Initialises the stdout tracer, and (if required) the OpenTelemetry service for the crate
    ///
    /// ## Arguments
    /// * `options` - The caller-specified instance of TracerOptions.
    /// * `service_name` - The name of the OpenTelemetry service to assign to the crate.
    /// * `module_name` - The name of the current module.
    ///
    /// ## Returns
    /// An instance of TracerEngine
    pub fn new(options: TracerOptions, service_name: &str) -> Self {
        let use_otel = options.otel_options.is_some();

        let stdout_tracer = tracing_subscriber::fmt::layer().with_writer(std::io::stdout);

        // if options.otel_options is provided then attempt to setup OtelTracer
        let (otel_tracer, otel_setup_error) = options
            .otel_options
            .map(
                |otel_options| match OtelTracer::<_>::new(otel_options, service_name) {
                    Ok(otel_tracer) => (Some(otel_tracer), None),
                    Err(e) => (None, Some(e)),
                },
            )
            .unwrap_or((None, None));
        // If otel_tracer did not work, update the use_otel variable
        let use_otel = use_otel && otel_tracer.is_some();
        let (otel_layer, otel_tracer_provider) = otel_tracer
            .map(|otel_tracer| (otel_tracer.layer, otel_tracer.tracer_provider))
            .unzip();

        // This filter is applied to the stdout tracer
        let log_filter = EnvFilter::from_default_env();

        let subscriber = tracing_subscriber::Registry::default()
            .with(stdout_tracer.with_filter(log_filter))
            .with(otel_layer);

        //  This is only called once, so will never panic
        tracing::subscriber::set_global_default(subscriber)
            .expect("tracing::subscriber::set_global_default should only be called once");

        Self {
            use_otel,
            otel_tracer_provider,
            otel_setup_error,
        }
    }

    /// Sets a span's parent to other_span
    pub fn set_span_parent_to(span: &Span, parent_span: &Span) {
        if let Err(e) = span.set_parent(parent_span.context()) {
            warn!("{e}");
        }
    }

    pub fn use_otel(&self) -> bool {
        self.use_otel
    }

    pub fn get_otel_setup_error(&self) -> Option<&ExporterBuildError> {
        self.otel_setup_error.as_ref()
    }
}

impl Drop for TracerEngine {
    fn drop(&mut self) {
        if let Some(otel_tracer_provider) = self.otel_tracer_provider.as_mut()
            && let Err(e) = otel_tracer_provider.shutdown()
        {
            warn!("{e}");
        }
    }
}
