//! Shared process telemetry bootstrap for the API and worker binaries:
//! tracing subscriber setup, optional Sentry error reporting, and optional
//! OTLP span export. Both `V2BOARD_SENTRY_DSN` and
//! `V2BOARD_OTEL_EXPORTER_OTLP_ENDPOINT` are entirely off unless set; an
//! invalid value warns and stays off rather than failing the service. The
//! `production` flag consistently drives both the Sentry `environment` tag
//! and the log format (JSON in production, human-readable `fmt` logs
//! otherwise) so the API and worker processes cannot independently drift on
//! what "production" means for logging.

use std::sync::atomic::{AtomicBool, Ordering};

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Process-lifetime telemetry state. Holding it keeps the Sentry client
/// alive; dropping it at the end of `main` flushes queued Sentry events and
/// shuts down the OTLP tracer provider so buffered spans export.
pub struct TelemetryGuard {
    _sentry: Option<sentry::ClientInitGuard>,
    otel: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.otel.take()
            && let Err(error) = provider.shutdown()
        {
            eprintln!("OTLP tracer provider shutdown did not flush cleanly: {error}");
        }
    }
}

static OTEL_ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether OTLP span export was initialized for this process. Gates
/// per-request W3C trace-context extraction so the disabled default costs
/// nothing.
pub fn otel_enabled() -> bool {
    OTEL_ENABLED.load(Ordering::Relaxed)
}

/// Initializes tracing plus optional Sentry error reporting and optional OTLP
/// span export. Must run before the tokio runtime starts: the OTLP batch
/// exporter constructs a blocking HTTP client, which panics inside one. The
/// caller holds the returned guard for the process lifetime.
///
/// `service_name` identifies the process for Sentry/OTel resource
/// attribution and doubles as the OTel tracer name (e.g. `"v2board-api"`,
/// `"v2board-worker"`). `default_env_filter` is the fallback `RUST_LOG`-style
/// directive used when the environment does not already specify one (via
/// `RUST_LOG`/`V2BOARD_LOG`, whichever `EnvFilter::try_from_default_env`
/// honors). `production` consistently controls both the Sentry `environment`
/// tag and the log format: JSON in production, human-readable `fmt` logs
/// otherwise.
pub fn init_tracing(
    service_name: &'static str,
    default_env_filter: &str,
    production: bool,
) -> TelemetryGuard {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_env_filter));
    let dsn = parse_sentry_dsn(std::env::var("V2BOARD_SENTRY_DSN").ok());
    let sentry_guard = dsn.as_ref().ok().and_then(Option::as_ref).map(|dsn| {
        sentry::init(sentry::ClientOptions {
            dsn: Some(dsn.clone()),
            release: sentry::release_name!(),
            environment: Some(if production { "production" } else { "local" }.into()),
            attach_stacktrace: true,
            ..Default::default()
        })
    });
    let otel = init_otel(service_name);
    let otel_provider = otel.as_ref().ok().and_then(Option::as_ref);
    // ERROR events become Sentry events and WARN/INFO become breadcrumbs
    // (the sentry-tracing default); without a client the layer is absent.
    let sentry_enabled = sentry_guard.is_some();
    if production {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_current_span(true)
                    .with_span_list(false),
            )
            .with(sentry_enabled.then(sentry::integrations::tracing::layer))
            .with(otel_provider.map(|provider| {
                use opentelemetry::trace::TracerProvider as _;
                tracing_opentelemetry::layer().with_tracer(provider.tracer(service_name))
            }))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .with(sentry_enabled.then(sentry::integrations::tracing::layer))
            .with(otel_provider.map(|provider| {
                use opentelemetry::trace::TracerProvider as _;
                tracing_opentelemetry::layer().with_tracer(provider.tracer(service_name))
            }))
            .init();
    }
    if let Err(error) = &dsn {
        tracing::warn!(%error, "V2BOARD_SENTRY_DSN is invalid; error reporting is disabled");
    }
    let otel = match otel {
        Ok(provider) => provider,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "V2BOARD_OTEL_EXPORTER_OTLP_ENDPOINT is invalid; span export is disabled"
            );
            None
        }
    };
    if otel.is_some() {
        OTEL_ENABLED.store(true, Ordering::Relaxed);
    }
    TelemetryGuard {
        _sentry: sentry_guard,
        otel,
    }
}

/// `Ok(None)` when the endpoint variable is absent or blank (export off).
/// When set, the W3C trace-context propagator becomes the process global so
/// incoming `traceparent` headers join the exported trace.
fn init_otel(
    service_name: &'static str,
) -> Result<Option<opentelemetry_sdk::trace::SdkTracerProvider>, String> {
    let Some(endpoint) = std::env::var("V2BOARD_OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
    else {
        return Ok(None);
    };
    let traces_endpoint = normalize_otlp_traces_endpoint(&endpoint)?;
    let exporter = {
        use opentelemetry_otlp::WithExportConfig;
        opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
            .with_endpoint(traces_endpoint)
            .build()
            .map_err(|error| error.to_string())?
    };
    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            opentelemetry_sdk::Resource::builder()
                .with_service_name(service_name)
                .build(),
        )
        .build();
    opentelemetry::global::set_text_map_propagator(
        opentelemetry_sdk::propagation::TraceContextPropagator::new(),
    );
    Ok(Some(provider))
}

/// Standard OTLP ergonomics: the operator supplies the base endpoint
/// (e.g. `http://127.0.0.1:4318`) and the traces signal path is appended,
/// unless a full signal URL was already given.
fn normalize_otlp_traces_endpoint(endpoint: &str) -> Result<String, String> {
    let url = url::Url::parse(endpoint)
        .map_err(|error| format!("endpoint is not a valid URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("endpoint must be an http(s) URL".to_owned());
    }
    let trimmed = endpoint.trim_end_matches('/');
    if trimmed.ends_with("/v1/traces") {
        Ok(trimmed.to_owned())
    } else {
        Ok(format!("{trimmed}/v1/traces"))
    }
}

/// `Ok(None)` when the variable is absent or blank (reporting off); `Err`
/// preserves the parse failure so it can be logged once tracing is up.
fn parse_sentry_dsn(
    raw: Option<String>,
) -> Result<Option<sentry::types::Dsn>, sentry::types::ParseDsnError> {
    match raw.as_deref().map(str::trim).filter(|raw| !raw.is_empty()) {
        Some(raw) => raw.parse().map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_otlp_traces_endpoint, parse_sentry_dsn};

    #[test]
    fn otlp_endpoints_normalize_to_the_traces_signal_url() {
        assert_eq!(
            normalize_otlp_traces_endpoint("http://127.0.0.1:4318"),
            Ok("http://127.0.0.1:4318/v1/traces".to_owned())
        );
        assert_eq!(
            normalize_otlp_traces_endpoint("https://otel.example.com/"),
            Ok("https://otel.example.com/v1/traces".to_owned())
        );
        assert_eq!(
            normalize_otlp_traces_endpoint("http://127.0.0.1:4318/v1/traces"),
            Ok("http://127.0.0.1:4318/v1/traces".to_owned())
        );
        assert!(normalize_otlp_traces_endpoint("not a url").is_err());
        assert!(normalize_otlp_traces_endpoint("grpc://127.0.0.1:4317").is_err());
    }

    #[test]
    fn sentry_reporting_is_off_without_a_dsn_and_on_parse_failure() {
        assert!(matches!(parse_sentry_dsn(None), Ok(None)));
        assert!(matches!(parse_sentry_dsn(Some(String::new())), Ok(None)));
        assert!(matches!(
            parse_sentry_dsn(Some("   ".to_string())),
            Ok(None)
        ));
        assert!(parse_sentry_dsn(Some("not a dsn".to_string())).is_err());
        let parsed = parse_sentry_dsn(Some(
            "https://f00d@o111111.ingest.sentry.io/2222".to_string(),
        ));
        assert!(matches!(parsed, Ok(Some(_))));
    }
}
