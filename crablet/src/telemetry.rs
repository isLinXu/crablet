use anyhow::Result;
#[cfg(feature = "telemetry")]
use opentelemetry::global;
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::propagation::TraceContextPropagator;
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::trace::{self, Sampler};
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::Resource;
#[cfg(feature = "telemetry")]
use opentelemetry_sdk::runtime;
#[cfg(feature = "telemetry")]
use opentelemetry::KeyValue;
#[cfg(feature = "telemetry")]
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};

pub fn init_telemetry(log_level: &str) -> Result<()> {
    #[cfg(feature = "telemetry")]
    {
        // Set global propagator
        global::set_text_map_propagator(TraceContextPropagator::new());

        // Check if OTEL_EXPORTER_OTLP_ENDPOINT is set, if not, skip OTEL setup or use stdout
        // For now, let's assume if it's set we use it.
        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();
        
        // Env filter for logs
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| log_level.into());

        if let Some(endpoint) = otlp_endpoint {
            println!("Initializing OpenTelemetry with endpoint: {}", endpoint);
            
            // Initialize OTLP pipeline
            let tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(
                    opentelemetry_otlp::new_exporter()
                        .tonic()
                        .with_endpoint(endpoint),
                )
                .with_trace_config(
                    trace::config()
                        .with_sampler(Sampler::AlwaysOn)
                        .with_resource(Resource::new(vec![
                            KeyValue::new("service.name", "crablet"),
                            KeyValue::new("service.version", "0.1.0"),
                        ])),
                )
                .install_batch(runtime::Tokio)?;

            // Create tracing layer
            let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

            // Standard stdout fmt layer
            let fmt_layer = tracing_subscriber::fmt::layer();

            // Register everything
            Registry::default()
                .with(env_filter)
                .with(fmt_layer)
                .with(telemetry)
                .init();
                
        } else {
            // Fallback to just stdout logging
            Registry::default()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer())
                .init();
        }
    }

    #[cfg(not(feature = "telemetry"))]
    {
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| log_level.into());
        Registry::default()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    Ok(())
}

pub fn shutdown_telemetry() {
    #[cfg(feature = "telemetry")]
    global::shutdown_tracer_provider();
}
