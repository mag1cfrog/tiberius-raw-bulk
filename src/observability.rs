//! Internal tracing contract for stable protocol observability.
//!
//! Stable protocol tracing in this crate follows one crate-owned target
//! namespace. Every stable event must include a structured `telemetry_event`
//! field with a stable dotted value. Stable spans must use stable dotted names,
//! and events emitted while a span is active should attach to that span through
//! normal tracing context propagation.
//!
//! Structured field names use snake_case. Count fields end in `_count`, byte
//! fields end in `_bytes`, and elapsed duration fields end in `_elapsed_ms`.
//! Info-level fields should avoid high-cardinality values and prefer booleans,
//! enums, counts, sizes, elapsed time, and protocol kind names.
//!
//! Stable default tracing must not include connection strings, credential
//! usernames, passwords, access tokens, raw SQL text, row values, parameter
//! values, raw packet bytes, certificate DER bytes, raw token debug output, or
//! arbitrary server-returned message text.

use crate::{client::AuthMethod, EncryptionLevel, Error};
use std::time::Duration;
use tracing::{Level, Span};

pub(crate) mod bulk_load;

/// Stable tracing target constants.
pub(crate) mod target {
    /// Crate-owned target for stable TDS protocol telemetry.
    pub(crate) const PROTOCOL: &str = "tiberius_raw_bulk::protocol";
}

/// Stable structured field name constants.
pub(crate) mod field {
    /// Field that identifies the stable event name.
    pub(crate) const TELEMETRY_EVENT: &str = "telemetry_event";
}

/// Stable span name constants.
pub(crate) mod span {
    /// Connection setup span for an already-open transport stream.
    pub(crate) const CONNECTION_CONNECT: &str = "protocol.connection.connect";

    /// TLS negotiation span.
    pub(crate) const TLS_NEGOTIATION: &str = "protocol.tls.negotiation";

    /// Login and authentication flow span.
    pub(crate) const LOGIN_FLOW: &str = "protocol.login.flow";

    /// Bulk-load request span.
    pub(crate) const BULK_LOAD_REQUEST: &str = "protocol.bulk_load.request";

    /// Smoke span used to validate the observability contract helper.
    pub(crate) const SMOKE: &str = "protocol.smoke";
}

/// Stable telemetry event name constants.
pub(crate) mod event {
    /// Connection setup span marker event.
    pub(crate) const CONNECTION_CONNECT: &str = "protocol.connection.connect";

    /// Connection setup started.
    pub(crate) const CONNECTION_SETUP_START: &str = "protocol.connection.setup.start";

    /// Connection setup completed.
    pub(crate) const CONNECTION_SETUP_COMPLETED: &str = "protocol.connection.setup.completed";

    /// Connection setup failed.
    pub(crate) const CONNECTION_SETUP_FAILED: &str = "protocol.connection.setup.failed";

    /// Prelogin started.
    pub(crate) const CONNECTION_PRELOGIN_START: &str = "protocol.connection.prelogin.start";

    /// Prelogin completed.
    pub(crate) const CONNECTION_PRELOGIN_COMPLETED: &str = "protocol.connection.prelogin.completed";

    /// Prelogin failed.
    pub(crate) const CONNECTION_PRELOGIN_FAILED: &str = "protocol.connection.prelogin.failed";

    /// TLS negotiation span marker event.
    pub(crate) const TLS_NEGOTIATION: &str = "protocol.tls.negotiation";

    /// TLS negotiation started.
    pub(crate) const TLS_NEGOTIATION_START: &str = "protocol.tls.negotiation.start";

    /// TLS negotiation completed.
    pub(crate) const TLS_NEGOTIATION_COMPLETED: &str = "protocol.tls.negotiation.completed";

    /// TLS negotiation failed.
    pub(crate) const TLS_NEGOTIATION_FAILED: &str = "protocol.tls.negotiation.failed";

    /// TLS trust configuration selected.
    pub(crate) const TLS_TRUST_CONFIG: &str = "protocol.tls.trust_config";

    /// TLS root certificates loaded.
    pub(crate) const TLS_ROOT_CERTIFICATES_LOADED: &str = "protocol.tls.root_certificates.loaded";

    /// TLS was downgraded after login.
    pub(crate) const TLS_POST_LOGIN_DOWNGRADED: &str = "protocol.tls.post_login.downgraded";

    /// Login flow span marker event.
    pub(crate) const LOGIN_FLOW: &str = "protocol.login.flow";

    /// Login flow started.
    pub(crate) const LOGIN_FLOW_START: &str = "protocol.login.flow.start";

    /// Login flow completed.
    pub(crate) const LOGIN_FLOW_COMPLETED: &str = "protocol.login.flow.completed";

    /// Login flow failed.
    pub(crate) const LOGIN_FLOW_FAILED: &str = "protocol.login.flow.failed";

    /// Bulk-load request span marker event.
    pub(crate) const BULK_LOAD_REQUEST: &str = "protocol.bulk_load.request";

    /// Bulk-load request started.
    pub(crate) const BULK_LOAD_REQUEST_START: &str = "protocol.bulk_load.request.start";

    /// Bulk-load request completed.
    pub(crate) const BULK_LOAD_REQUEST_COMPLETED: &str = "protocol.bulk_load.request.completed";

    /// Bulk-load request failed.
    pub(crate) const BULK_LOAD_REQUEST_FAILED: &str = "protocol.bulk_load.request.failed";

    /// Bulk-load packet write summary.
    pub(crate) const BULK_LOAD_PACKET_WRITTEN: &str = "protocol.bulk_load.packet.written";

    /// Bulk-load flush completed.
    pub(crate) const BULK_LOAD_FLUSH_COMPLETED: &str = "protocol.bulk_load.flush.completed";

    /// Bulk-load flush failed.
    pub(crate) const BULK_LOAD_FLUSH_FAILED: &str = "protocol.bulk_load.flush.failed";

    /// Smoke event used to validate the observability contract helper.
    pub(crate) const SMOKE: &str = "protocol.smoke";
}

/// Creates the stable span for connection setup on an already-open transport.
pub(crate) fn connection_connect_span(
    requested_encryption: EncryptionLevel,
    fed_auth_required: bool,
) -> Span {
    tracing::span!(
        target: target::PROTOCOL,
        Level::INFO,
        span::CONNECTION_CONNECT,
        telemetry_event = event::CONNECTION_CONNECT,
        phase = "connection",
        operation = "connect",
        requested_encryption = encryption_level_name(requested_encryption),
        fed_auth_required = fed_auth_required,
    )
}

/// Returns the safe TLS backend name for the active feature path.
#[cfg(feature = "rustls")]
pub(crate) fn tls_backend_name() -> &'static str {
    "rustls"
}

/// Returns the safe TLS backend name for the active feature path.
#[cfg(all(not(feature = "rustls"), feature = "native-tls"))]
pub(crate) fn tls_backend_name() -> &'static str {
    "native_tls"
}

/// Returns the safe TLS backend name for the active feature path.
#[cfg(all(
    not(feature = "rustls"),
    not(feature = "native-tls"),
    feature = "vendored-openssl"
))]
pub(crate) fn tls_backend_name() -> &'static str {
    "vendored_openssl"
}

/// Returns the safe TLS backend name when no TLS backend is compiled in.
#[cfg(not(any(
    feature = "rustls",
    feature = "native-tls",
    feature = "vendored-openssl"
)))]
pub(crate) fn tls_backend_name() -> &'static str {
    "none"
}

/// Returns a safe authentication method category.
pub(crate) fn auth_method_category(auth: &AuthMethod) -> &'static str {
    match auth {
        AuthMethod::SqlServer(_) => "sql_server",
        #[cfg(any(all(windows, feature = "winauth"), doc))]
        AuthMethod::Windows(_) => "windows",
        #[cfg(any(
            all(windows, feature = "winauth"),
            all(unix, feature = "integrated-auth-gssapi"),
            doc
        ))]
        AuthMethod::Integrated => "integrated",
        AuthMethod::AADToken(_) => "aad_token",
        AuthMethod::None => "none",
    }
}

/// Creates the stable TLS negotiation span.
pub(crate) fn tls_negotiation_span(encryption: EncryptionLevel, tls_backend: &'static str) -> Span {
    tracing::span!(
        target: target::PROTOCOL,
        Level::INFO,
        span::TLS_NEGOTIATION,
        telemetry_event = event::TLS_NEGOTIATION,
        phase = "tls",
        operation = "negotiate",
        tls_backend = tls_backend,
        encryption = encryption_level_name(encryption),
    )
}

/// Creates the stable login/auth flow span.
pub(crate) fn login_flow_span(encryption: EncryptionLevel, auth_method: &'static str) -> Span {
    tracing::span!(
        target: target::PROTOCOL,
        Level::INFO,
        span::LOGIN_FLOW,
        telemetry_event = event::LOGIN_FLOW,
        phase = "login",
        operation = "authenticate",
        auth_method = auth_method,
        encryption = encryption_level_name(encryption),
    )
}

/// Emits the stable connection setup start event.
pub(crate) fn emit_connection_setup_start(
    requested_encryption: EncryptionLevel,
    fed_auth_required: bool,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::CONNECTION_SETUP_START,
        phase = "connection",
        operation = "connect",
        status = "started",
        requested_encryption = encryption_level_name(requested_encryption),
        fed_auth_required = fed_auth_required,
    );
}

/// Emits the stable connection setup completed event.
pub(crate) fn emit_connection_setup_completed(
    setup_elapsed: Duration,
    negotiated_encryption: EncryptionLevel,
    packet_size: u32,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::CONNECTION_SETUP_COMPLETED,
        phase = "connection",
        operation = "connect",
        status = "completed",
        connection_elapsed_ms = duration_ms(setup_elapsed),
        negotiated_encryption = encryption_level_name(negotiated_encryption),
        packet_size_bytes = u64::from(packet_size),
    );
}

/// Emits the stable connection setup failed event.
pub(crate) fn emit_connection_setup_failed(setup_elapsed: Duration, error: &Error) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::WARN,
        telemetry_event = event::CONNECTION_SETUP_FAILED,
        phase = "connection",
        operation = "connect",
        status = "failed",
        connection_elapsed_ms = duration_ms(setup_elapsed),
        error_category = error_category(error),
    );
}

/// Emits the stable prelogin start event.
pub(crate) fn emit_prelogin_start(requested_encryption: EncryptionLevel, fed_auth_required: bool) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::CONNECTION_PRELOGIN_START,
        phase = "prelogin",
        operation = "prelogin",
        status = "started",
        requested_encryption = encryption_level_name(requested_encryption),
        fed_auth_required = fed_auth_required,
    );
}

/// Emits the stable prelogin completed event.
pub(crate) fn emit_prelogin_completed(
    prelogin_elapsed: Duration,
    server_encryption: EncryptionLevel,
    server_fed_auth_required: bool,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::CONNECTION_PRELOGIN_COMPLETED,
        phase = "prelogin",
        operation = "prelogin",
        status = "completed",
        prelogin_elapsed_ms = duration_ms(prelogin_elapsed),
        server_encryption = encryption_level_name(server_encryption),
        server_fed_auth_required = server_fed_auth_required,
    );
}

/// Emits the stable prelogin failed event.
pub(crate) fn emit_prelogin_failed(prelogin_elapsed: Duration, error: &Error) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::WARN,
        telemetry_event = event::CONNECTION_PRELOGIN_FAILED,
        phase = "prelogin",
        operation = "prelogin",
        status = "failed",
        prelogin_elapsed_ms = duration_ms(prelogin_elapsed),
        error_category = error_category(error),
    );
}

/// Emits the stable TLS negotiation start event.
pub(crate) fn emit_tls_negotiation_start(encryption: EncryptionLevel, tls_backend: &'static str) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::TLS_NEGOTIATION_START,
        phase = "tls",
        operation = "negotiate",
        status = "started",
        tls_backend = tls_backend,
        encryption = encryption_level_name(encryption),
    );
}

/// Emits the stable TLS negotiation completed event.
pub(crate) fn emit_tls_negotiation_completed(
    tls_elapsed: Duration,
    encryption: EncryptionLevel,
    tls_backend: &'static str,
    tls_used: bool,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::TLS_NEGOTIATION_COMPLETED,
        phase = "tls",
        operation = "negotiate",
        status = "completed",
        tls_elapsed_ms = duration_ms(tls_elapsed),
        tls_backend = tls_backend,
        encryption = encryption_level_name(encryption),
        tls_used = tls_used,
    );
}

/// Emits the stable TLS negotiation failed event.
pub(crate) fn emit_tls_negotiation_failed(
    tls_elapsed: Duration,
    encryption: EncryptionLevel,
    tls_backend: &'static str,
    error: &Error,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::WARN,
        telemetry_event = event::TLS_NEGOTIATION_FAILED,
        phase = "tls",
        operation = "negotiate",
        status = "failed",
        tls_elapsed_ms = duration_ms(tls_elapsed),
        tls_backend = tls_backend,
        encryption = encryption_level_name(encryption),
        error_category = error_category(error),
    );
}

/// Emits the stable TLS trust configuration event.
pub(crate) fn emit_tls_trust_config(
    tls_backend: &'static str,
    trust_mode: &'static str,
    certificate_validation: bool,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::TLS_TRUST_CONFIG,
        phase = "tls",
        operation = "configure_trust",
        tls_backend = tls_backend,
        trust_mode = trust_mode,
        certificate_validation = certificate_validation,
    );
}

/// Emits the stable root certificate loading summary event.
pub(crate) fn emit_tls_root_certificates_loaded(
    tls_backend: &'static str,
    valid_count: u64,
    invalid_count: u64,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::TLS_ROOT_CERTIFICATES_LOADED,
        phase = "tls",
        operation = "load_root_certificates",
        tls_backend = tls_backend,
        valid_count = valid_count,
        invalid_count = invalid_count,
    );
}

/// Emits the stable post-login TLS downgrade event.
pub(crate) fn emit_tls_post_login_downgraded(encryption: EncryptionLevel) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::WARN,
        telemetry_event = event::TLS_POST_LOGIN_DOWNGRADED,
        phase = "tls",
        operation = "post_login_encryption",
        status = "downgraded",
        encryption = encryption_level_name(encryption),
        tls_used = false,
        downgraded = true,
    );
}

/// Emits the stable login flow start event.
pub(crate) fn emit_login_flow_start(encryption: EncryptionLevel, auth_method: &'static str) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::LOGIN_FLOW_START,
        phase = "login",
        operation = "authenticate",
        status = "started",
        encryption = encryption_level_name(encryption),
        auth_method = auth_method,
    );
}

/// Emits the stable login flow completed event.
pub(crate) fn emit_login_flow_completed(
    login_elapsed: Duration,
    encryption: EncryptionLevel,
    auth_method: &'static str,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::LOGIN_FLOW_COMPLETED,
        phase = "login",
        operation = "authenticate",
        status = "completed",
        login_elapsed_ms = duration_ms(login_elapsed),
        encryption = encryption_level_name(encryption),
        auth_method = auth_method,
    );
}

/// Emits the stable login flow failed event.
pub(crate) fn emit_login_flow_failed(
    login_elapsed: Duration,
    encryption: EncryptionLevel,
    auth_method: &'static str,
    error: &Error,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::WARN,
        telemetry_event = event::LOGIN_FLOW_FAILED,
        phase = "login",
        operation = "authenticate",
        status = "failed",
        login_elapsed_ms = duration_ms(login_elapsed),
        encryption = encryption_level_name(encryption),
        auth_method = auth_method,
        error_category = error_category(error),
    );
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn encryption_level_name(encryption: EncryptionLevel) -> &'static str {
    match encryption {
        EncryptionLevel::Off => "off",
        EncryptionLevel::On => "on",
        EncryptionLevel::NotSupported => "not_supported",
        EncryptionLevel::Required => "required",
    }
}

fn error_category(error: &Error) -> &'static str {
    match error {
        Error::Io { .. } => "io",
        Error::Protocol(_) => "protocol",
        Error::Encoding(_) => "encoding",
        Error::Conversion(_) => "conversion",
        Error::Utf8 => "utf8",
        Error::Utf16 => "utf16",
        Error::ParseInt(_) => "parse_int",
        Error::Server(_) => "server",
        Error::Tls(_) => "tls",
        #[cfg(any(all(unix, feature = "integrated-auth-gssapi"), doc))]
        Error::Gssapi(_) => "gssapi",
        Error::Routing { .. } => "routing",
        Error::BulkInput(_) => "bulk_input",
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::{event, field, span, target};
    use std::{
        collections::BTreeMap,
        fmt,
        future::Future,
        sync::{Arc, Mutex, MutexGuard, OnceLock},
    };
    use tracing::{field::Visit, instrument::WithSubscriber, Event, Id, Level, Subscriber};
    use tracing_subscriber::{
        layer::{Context, Layer},
        prelude::*,
        registry::LookupSpan,
    };

    static CAPTURE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    /// Captured tracing record kind.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum CapturedRecordKind {
        /// A span creation record.
        Span,
        /// An event record.
        Event,
    }

    /// Captured span or event metadata and structured fields.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) struct CapturedRecord {
        pub(crate) kind: CapturedRecordKind,
        pub(crate) name: String,
        pub(crate) target: String,
        pub(crate) level: Level,
        pub(crate) parent_span_name: Option<String>,
        pub(crate) fields: BTreeMap<String, String>,
    }

    impl CapturedRecord {
        /// Returns the captured string form for a structured field.
        pub(crate) fn field(&self, name: &str) -> Option<&str> {
            self.fields.get(name).map(String::as_str)
        }

        /// Asserts that a structured field has the expected captured value.
        pub(crate) fn assert_field(&self, name: &str, expected: &str) {
            assert_eq!(Some(expected), self.field(name));
        }

        /// Returns true when this record contains any captured text.
        pub(crate) fn contains_text(&self, needle: &str) -> bool {
            self.name.contains(needle)
                || self.target.contains(needle)
                || self
                    .parent_span_name
                    .as_deref()
                    .is_some_and(|parent| parent.contains(needle))
                || self
                    .fields
                    .iter()
                    .any(|(name, value)| name.contains(needle) || value.contains(needle))
        }
    }

    /// Records captured by a scoped tracing subscriber.
    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    pub(crate) struct CapturedRecords {
        records: Vec<CapturedRecord>,
    }

    impl CapturedRecords {
        /// Returns all captured records in capture order.
        pub(crate) fn records(&self) -> &[CapturedRecord] {
            &self.records
        }

        /// Finds the first event with the requested `telemetry_event` value.
        pub(crate) fn event(&self, telemetry_event: &str) -> Option<&CapturedRecord> {
            self.records.iter().find(|record| {
                record.kind == CapturedRecordKind::Event
                    && record.field(field::TELEMETRY_EVENT) == Some(telemetry_event)
            })
        }

        /// Finds the first span with the requested span name.
        pub(crate) fn span(&self, span_name: &str) -> Option<&CapturedRecord> {
            self.records
                .iter()
                .find(|record| record.kind == CapturedRecordKind::Span && record.name == span_name)
        }

        /// Asserts that no captured record contains any forbidden text.
        pub(crate) fn assert_no_forbidden_text(&self, forbidden: &[&str]) {
            for needle in forbidden {
                if let Some(record) = self
                    .records
                    .iter()
                    .find(|record| record.contains_text(needle))
                {
                    panic!("forbidden tracing text `{needle}` found in {record:?}");
                }
            }
        }
    }

    /// Captures tracing records with a scoped subscriber around a sync closure.
    pub(crate) fn capture<F, T>(f: F) -> (T, CapturedRecords)
    where
        F: FnOnce() -> T,
    {
        let _guard = capture_guard();
        let (subscriber, layer) = capture_subscriber();

        let output = tracing::subscriber::with_default(subscriber, || {
            tracing_core::callsite::rebuild_interest_cache();
            let output = f();
            tracing_core::callsite::rebuild_interest_cache();
            output
        });
        tracing_core::callsite::rebuild_interest_cache();

        (output, layer.records())
    }

    /// Captures tracing records with a scoped subscriber around an async future.
    #[allow(clippy::await_holding_lock)]
    pub(crate) async fn capture_async<Fut>(future: Fut) -> (Fut::Output, CapturedRecords)
    where
        Fut: Future,
    {
        let _guard = capture_guard();
        let (subscriber, layer) = capture_subscriber();
        let dispatch = tracing::Dispatch::new(subscriber);

        let output = async {
            tracing_core::callsite::rebuild_interest_cache();
            let output = future.await;
            tracing_core::callsite::rebuild_interest_cache();
            output
        }
        .with_subscriber(dispatch)
        .await;
        tracing_core::callsite::rebuild_interest_cache();

        (output, layer.records())
    }

    /// Runs a closure with an explicit scoped no-op subscriber.
    pub(crate) fn with_no_subscriber<F, T>(f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let _guard = capture_guard();

        let output =
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                tracing_core::callsite::rebuild_interest_cache();
                let output = f();
                tracing_core::callsite::rebuild_interest_cache();
                output
            });
        tracing_core::callsite::rebuild_interest_cache();

        output
    }

    /// Emits one stable smoke span and event for observability tests.
    pub(crate) fn emit_smoke_trace() {
        let smoke = tracing::span!(
            target: target::PROTOCOL,
            Level::TRACE,
            span::SMOKE,
            telemetry_event = event::SMOKE,
            row_count = 1_u64,
            packet_bytes = 32_u64,
            write_elapsed_ms = 3_u64,
            encrypted = false,
        );

        let _entered = smoke.enter();

        tracing::event!(
            target: target::PROTOCOL,
            Level::TRACE,
            telemetry_event = event::SMOKE,
            row_count = 1_u64,
            packet_bytes = 32_u64,
            write_elapsed_ms = 3_u64,
            encrypted = false,
        );
    }

    fn capture_guard() -> MutexGuard<'static, ()> {
        CAPTURE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn capture_subscriber() -> (impl Subscriber + Send + Sync + 'static, CaptureLayer) {
        let layer = CaptureLayer::default();
        let subscriber = tracing_subscriber::registry().with(layer.clone());

        (subscriber, layer)
    }

    #[derive(Clone, Debug, Default)]
    struct CaptureLayer {
        records: Arc<Mutex<Vec<CapturedRecord>>>,
    }

    impl CaptureLayer {
        fn records(&self) -> CapturedRecords {
            let records = self
                .records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();

            CapturedRecords { records }
        }

        fn push(&self, record: CapturedRecord) {
            self.records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(record);
        }
    }

    impl<S> Layer<S> for CaptureLayer
    where
        S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    {
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            _id: &Id,
            ctx: Context<'_, S>,
        ) {
            let metadata = attrs.metadata();
            let mut visitor = FieldVisitor::default();
            attrs.record(&mut visitor);

            self.push(CapturedRecord {
                kind: CapturedRecordKind::Span,
                name: metadata.name().to_string(),
                target: metadata.target().to_string(),
                level: *metadata.level(),
                parent_span_name: span_parent_name(attrs.parent(), attrs.is_contextual(), &ctx),
                fields: visitor.fields,
            });
        }

        fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
            let metadata = event.metadata();
            let mut visitor = FieldVisitor::default();
            event.record(&mut visitor);

            self.push(CapturedRecord {
                kind: CapturedRecordKind::Event,
                name: metadata.name().to_string(),
                target: metadata.target().to_string(),
                level: *metadata.level(),
                parent_span_name: span_parent_name(event.parent(), event.is_contextual(), &ctx),
                fields: visitor.fields,
            });
        }
    }

    fn span_parent_name<S>(
        explicit_parent: Option<&Id>,
        is_contextual: bool,
        ctx: &Context<'_, S>,
    ) -> Option<String>
    where
        S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    {
        if let Some(parent) = explicit_parent.and_then(|id| ctx.span(id)) {
            return Some(parent.metadata().name().to_string());
        }

        if is_contextual {
            return ctx
                .current_span()
                .id()
                .and_then(|id| ctx.span(id))
                .map(|span| span.metadata().name().to_string());
        }

        None
    }

    #[derive(Debug, Default)]
    struct FieldVisitor {
        fields: BTreeMap<String, String>,
    }

    impl FieldVisitor {
        fn insert(&mut self, field: &tracing::field::Field, value: String) {
            self.fields.insert(field.name().to_string(), value);
        }
    }

    impl Visit for FieldVisitor {
        fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
            self.insert(field, value.to_string());
        }

        fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
            self.insert(field, value.to_string());
        }

        fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
            self.insert(field, value.to_string());
        }

        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            self.insert(field, value.to_string());
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
            self.insert(field, format!("{value:?}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{EncryptionLevel, Error};

    use super::{
        event, target,
        test_support::{self, CapturedRecordKind},
    };
    use std::{borrow::Cow, time::Duration};
    use tracing::Level;

    #[test]
    fn scoped_capture_records_stable_smoke_span_and_event() {
        let (_output, records) = test_support::capture(test_support::emit_smoke_trace);

        let smoke_span = records
            .span(super::span::SMOKE)
            .unwrap_or_else(|| panic!("missing smoke span in {records:?}"));
        assert_eq!(CapturedRecordKind::Span, smoke_span.kind);
        assert_eq!(target::PROTOCOL, smoke_span.target);
        assert_eq!(Level::TRACE, smoke_span.level);
        assert_eq!(
            Some(event::SMOKE),
            smoke_span.field(super::field::TELEMETRY_EVENT)
        );

        let smoke_event = records
            .event(event::SMOKE)
            .unwrap_or_else(|| panic!("missing smoke event in {records:?}"));
        assert_eq!(target::PROTOCOL, smoke_event.target);
        assert_eq!(Level::TRACE, smoke_event.level);
        assert_eq!(
            Some(super::span::SMOKE),
            smoke_event.parent_span_name.as_deref()
        );
        smoke_event.assert_field("row_count", "1");
        smoke_event.assert_field("packet_bytes", "32");
        smoke_event.assert_field("write_elapsed_ms", "3");
        smoke_event.assert_field("encrypted", "false");
    }

    #[tokio::test]
    async fn scoped_capture_records_async_smoke_event() {
        let (output, records) = test_support::capture_async(async {
            test_support::emit_smoke_trace();
            7_u8
        })
        .await;

        assert_eq!(7, output);
        let smoke_event = records
            .event(event::SMOKE)
            .unwrap_or_else(|| panic!("missing smoke event in {records:?}"));
        smoke_event.assert_field("row_count", "1");
    }

    #[test]
    fn smoke_event_succeeds_without_subscriber() {
        test_support::with_no_subscriber(test_support::emit_smoke_trace);
    }

    #[test]
    fn scoped_capture_preserves_active_caller_parent_span() {
        let (_output, records) = test_support::capture(|| {
            let caller = tracing::span!(Level::INFO, "caller.request");
            let _entered = caller.enter();

            test_support::emit_smoke_trace();
        });

        let smoke_span = records
            .span(super::span::SMOKE)
            .unwrap_or_else(|| panic!("missing smoke span in {records:?}"));
        assert_eq!(
            Some("caller.request"),
            smoke_span.parent_span_name.as_deref()
        );

        let smoke_event = records
            .event(event::SMOKE)
            .unwrap_or_else(|| panic!("missing smoke event in {records:?}"));
        assert_eq!(
            Some(super::span::SMOKE),
            smoke_event.parent_span_name.as_deref()
        );
    }

    #[test]
    fn connection_prelogin_helpers_emit_structured_fields() {
        let (_output, records) = test_support::capture(|| {
            let connection = super::connection_connect_span(EncryptionLevel::Required, true);
            let _entered = connection.enter();

            super::emit_connection_setup_start(EncryptionLevel::Required, true);
            super::emit_prelogin_start(EncryptionLevel::Required, true);
            super::emit_prelogin_completed(Duration::from_millis(4), EncryptionLevel::On, true);
            super::emit_connection_setup_completed(
                Duration::from_millis(9),
                EncryptionLevel::On,
                4096,
            );
        });

        let connection_span = records
            .span(super::span::CONNECTION_CONNECT)
            .unwrap_or_else(|| panic!("missing connection span in {records:?}"));
        assert_eq!(CapturedRecordKind::Span, connection_span.kind);
        assert_eq!(target::PROTOCOL, connection_span.target);
        assert_eq!(Level::INFO, connection_span.level);
        connection_span.assert_field(super::field::TELEMETRY_EVENT, event::CONNECTION_CONNECT);
        connection_span.assert_field("phase", "connection");
        connection_span.assert_field("operation", "connect");
        connection_span.assert_field("requested_encryption", "required");
        connection_span.assert_field("fed_auth_required", "true");

        let connection_start = records
            .event(event::CONNECTION_SETUP_START)
            .unwrap_or_else(|| panic!("missing connection start event in {records:?}"));
        connection_start.assert_field("status", "started");
        connection_start.assert_field("requested_encryption", "required");
        connection_start.assert_field("fed_auth_required", "true");

        let prelogin_start = records
            .event(event::CONNECTION_PRELOGIN_START)
            .unwrap_or_else(|| panic!("missing prelogin start event in {records:?}"));
        prelogin_start.assert_field("phase", "prelogin");
        prelogin_start.assert_field("operation", "prelogin");
        prelogin_start.assert_field("status", "started");

        let prelogin_completed = records
            .event(event::CONNECTION_PRELOGIN_COMPLETED)
            .unwrap_or_else(|| panic!("missing prelogin completed event in {records:?}"));
        prelogin_completed.assert_field("prelogin_elapsed_ms", "4");
        prelogin_completed.assert_field("server_encryption", "on");
        prelogin_completed.assert_field("server_fed_auth_required", "true");

        let connection_completed = records
            .event(event::CONNECTION_SETUP_COMPLETED)
            .unwrap_or_else(|| panic!("missing connection completed event in {records:?}"));
        connection_completed.assert_field("connection_elapsed_ms", "9");
        connection_completed.assert_field("negotiated_encryption", "on");
        connection_completed.assert_field("packet_size_bytes", "4096");
    }

    #[test]
    fn connection_prelogin_helpers_succeed_without_subscriber() {
        test_support::with_no_subscriber(|| {
            let connection = super::connection_connect_span(EncryptionLevel::Off, false);
            let _entered = connection.enter();

            super::emit_connection_setup_start(EncryptionLevel::Off, false);
            super::emit_prelogin_start(EncryptionLevel::Off, false);
            super::emit_prelogin_failed(
                Duration::from_millis(1),
                &Error::Protocol(Cow::Borrowed("protocol failure")),
            );
            super::emit_connection_setup_failed(
                Duration::from_millis(2),
                &Error::Protocol(Cow::Borrowed("protocol failure")),
            );
        });
    }

    #[test]
    fn connection_prelogin_helpers_preserve_active_caller_parent_span() {
        let (_output, records) = test_support::capture(|| {
            let caller = tracing::span!(Level::INFO, "caller.request");
            let _caller_entered = caller.enter();

            let connection = super::connection_connect_span(EncryptionLevel::Required, false);
            let _connection_entered = connection.enter();

            super::emit_prelogin_start(EncryptionLevel::Required, false);
        });

        let connection_span = records
            .span(super::span::CONNECTION_CONNECT)
            .unwrap_or_else(|| panic!("missing connection span in {records:?}"));
        assert_eq!(
            Some("caller.request"),
            connection_span.parent_span_name.as_deref()
        );

        let prelogin_start = records
            .event(event::CONNECTION_PRELOGIN_START)
            .unwrap_or_else(|| panic!("missing prelogin start event in {records:?}"));
        assert_eq!(
            Some(super::span::CONNECTION_CONNECT),
            prelogin_start.parent_span_name.as_deref()
        );
    }

    #[test]
    fn connection_prelogin_helpers_do_not_emit_forbidden_text() {
        let forbidden = [
            "Server=tcp:example.database.windows.net;User ID=alice;Password=secret",
            "alice",
            "password=secret",
            "Bearer secret-token",
            "SELECT * FROM sensitive_table WHERE ssn = '123-45-6789'",
            "app-secret-name",
            "database-secret-name",
        ];

        let (_output, records) = test_support::capture(|| {
            let connection = super::connection_connect_span(EncryptionLevel::Required, true);
            let _entered = connection.enter();

            super::emit_connection_setup_start(EncryptionLevel::Required, true);
            super::emit_prelogin_start(EncryptionLevel::Required, true);
            super::emit_prelogin_failed(
                Duration::from_millis(3),
                &Error::Protocol(Cow::Borrowed(
                    "Server=tcp:example.database.windows.net;User ID=alice;Password=secret",
                )),
            );
            super::emit_connection_setup_failed(
                Duration::from_millis(5),
                &Error::Protocol(Cow::Borrowed(
                    "SELECT * FROM sensitive_table WHERE ssn = '123-45-6789'",
                )),
            );
        });

        records.assert_no_forbidden_text(&forbidden);

        let prelogin_failed = records
            .event(event::CONNECTION_PRELOGIN_FAILED)
            .unwrap_or_else(|| panic!("missing prelogin failed event in {records:?}"));
        prelogin_failed.assert_field("error_category", "protocol");
    }

    #[test]
    fn tls_login_helpers_emit_structured_fields() {
        let (_output, records) = test_support::capture(|| {
            let tls = super::tls_negotiation_span(EncryptionLevel::Required, "rustls");
            let _tls_entered = tls.enter();

            super::emit_tls_negotiation_start(EncryptionLevel::Required, "rustls");
            super::emit_tls_trust_config("rustls", "default", true);
            super::emit_tls_root_certificates_loaded("rustls", 100, 2);
            super::emit_tls_negotiation_completed(
                Duration::from_millis(8),
                EncryptionLevel::On,
                "rustls",
                true,
            );

            let login = super::login_flow_span(EncryptionLevel::On, "sql_server");
            let _login_entered = login.enter();

            super::emit_login_flow_start(EncryptionLevel::On, "sql_server");
            super::emit_login_flow_completed(
                Duration::from_millis(11),
                EncryptionLevel::On,
                "sql_server",
            );
        });

        let tls_span = records
            .span(super::span::TLS_NEGOTIATION)
            .unwrap_or_else(|| panic!("missing tls span in {records:?}"));
        tls_span.assert_field(super::field::TELEMETRY_EVENT, event::TLS_NEGOTIATION);
        tls_span.assert_field("phase", "tls");
        tls_span.assert_field("operation", "negotiate");
        tls_span.assert_field("tls_backend", "rustls");
        tls_span.assert_field("encryption", "required");

        let trust = records
            .event(event::TLS_TRUST_CONFIG)
            .unwrap_or_else(|| panic!("missing tls trust event in {records:?}"));
        trust.assert_field("trust_mode", "default");
        trust.assert_field("certificate_validation", "true");

        let roots = records
            .event(event::TLS_ROOT_CERTIFICATES_LOADED)
            .unwrap_or_else(|| panic!("missing tls root event in {records:?}"));
        roots.assert_field("valid_count", "100");
        roots.assert_field("invalid_count", "2");

        let tls_completed = records
            .event(event::TLS_NEGOTIATION_COMPLETED)
            .unwrap_or_else(|| panic!("missing tls completed event in {records:?}"));
        tls_completed.assert_field("tls_elapsed_ms", "8");
        tls_completed.assert_field("tls_used", "true");

        let login_span = records
            .span(super::span::LOGIN_FLOW)
            .unwrap_or_else(|| panic!("missing login span in {records:?}"));
        login_span.assert_field(super::field::TELEMETRY_EVENT, event::LOGIN_FLOW);
        login_span.assert_field("auth_method", "sql_server");

        let login_completed = records
            .event(event::LOGIN_FLOW_COMPLETED)
            .unwrap_or_else(|| panic!("missing login completed event in {records:?}"));
        login_completed.assert_field("login_elapsed_ms", "11");
        login_completed.assert_field("auth_method", "sql_server");
    }

    #[test]
    fn tls_login_helpers_succeed_without_subscriber() {
        test_support::with_no_subscriber(|| {
            let tls = super::tls_negotiation_span(EncryptionLevel::Required, "native_tls");
            let _tls_entered = tls.enter();

            super::emit_tls_negotiation_start(EncryptionLevel::Required, "native_tls");
            super::emit_tls_negotiation_failed(
                Duration::from_millis(2),
                EncryptionLevel::Required,
                "native_tls",
                &Error::Tls("certificate PEM secret".to_string()),
            );
            super::emit_tls_post_login_downgraded(EncryptionLevel::Off);

            let login = super::login_flow_span(EncryptionLevel::Off, "aad_token");
            let _login_entered = login.enter();

            super::emit_login_flow_start(EncryptionLevel::Off, "aad_token");
            super::emit_login_flow_failed(
                Duration::from_millis(4),
                EncryptionLevel::Off,
                "aad_token",
                &Error::Protocol(Cow::Borrowed("Bearer secret-token")),
            );
        });
    }

    #[test]
    fn tls_login_helpers_preserve_active_caller_parent_span() {
        let (_output, records) = test_support::capture(|| {
            let caller = tracing::span!(Level::INFO, "caller.request");
            let _caller_entered = caller.enter();

            let tls = super::tls_negotiation_span(EncryptionLevel::Required, "rustls");
            let _tls_entered = tls.enter();
            super::emit_tls_negotiation_start(EncryptionLevel::Required, "rustls");

            let login = super::login_flow_span(EncryptionLevel::On, "integrated");
            let _login_entered = login.enter();
            super::emit_login_flow_start(EncryptionLevel::On, "integrated");
        });

        let tls_span = records
            .span(super::span::TLS_NEGOTIATION)
            .unwrap_or_else(|| panic!("missing tls span in {records:?}"));
        assert_eq!(Some("caller.request"), tls_span.parent_span_name.as_deref());

        let tls_start = records
            .event(event::TLS_NEGOTIATION_START)
            .unwrap_or_else(|| panic!("missing tls start event in {records:?}"));
        assert_eq!(
            Some(super::span::TLS_NEGOTIATION),
            tls_start.parent_span_name.as_deref()
        );

        let login_span = records
            .span(super::span::LOGIN_FLOW)
            .unwrap_or_else(|| panic!("missing login span in {records:?}"));
        assert_eq!(
            Some(super::span::TLS_NEGOTIATION),
            login_span.parent_span_name.as_deref()
        );

        let login_start = records
            .event(event::LOGIN_FLOW_START)
            .unwrap_or_else(|| panic!("missing login start event in {records:?}"));
        assert_eq!(
            Some(super::span::LOGIN_FLOW),
            login_start.parent_span_name.as_deref()
        );
    }

    #[test]
    fn tls_login_helpers_do_not_emit_forbidden_text() {
        let forbidden = [
            "-----BEGIN CERTIFICATE-----",
            "MIICsecretDER",
            "DOMAIN",
            "alice",
            "password=secret",
            "Bearer secret-token",
            "SSPI_PAYLOAD_BYTES",
            "GSSAPI_PAYLOAD_BYTES",
            "Server=tcp:example.database.windows.net",
            "database-secret-name",
            "app-secret-name",
        ];

        let (_output, records) = test_support::capture(|| {
            let tls = super::tls_negotiation_span(EncryptionLevel::Required, "rustls");
            let _tls_entered = tls.enter();

            super::emit_tls_trust_config("rustls", "ca_certificate", true);
            super::emit_tls_root_certificates_loaded("rustls", 1, 1);
            super::emit_tls_negotiation_failed(
                Duration::from_millis(3),
                EncryptionLevel::Required,
                "rustls",
                &Error::Tls("-----BEGIN CERTIFICATE----- MIICsecretDER".to_string()),
            );

            let login = super::login_flow_span(EncryptionLevel::On, "windows");
            let _login_entered = login.enter();

            super::emit_login_flow_failed(
                Duration::from_millis(5),
                EncryptionLevel::On,
                "windows",
                &Error::Protocol(Cow::Borrowed(
                    "DOMAIN alice password=secret Bearer secret-token SSPI_PAYLOAD_BYTES",
                )),
            );
        });

        records.assert_no_forbidden_text(&forbidden);

        let tls_failed = records
            .event(event::TLS_NEGOTIATION_FAILED)
            .unwrap_or_else(|| panic!("missing tls failed event in {records:?}"));
        tls_failed.assert_field("error_category", "tls");

        let login_failed = records
            .event(event::LOGIN_FLOW_FAILED)
            .unwrap_or_else(|| panic!("missing login failed event in {records:?}"));
        login_failed.assert_field("error_category", "protocol");
    }

    #[test]
    #[should_panic(expected = "forbidden tracing text")]
    fn forbidden_text_assertion_fails_on_leaked_text() {
        let leaked = "UnsafeServerMessage42";
        let (_output, records) = test_support::capture(|| {
            tracing::event!(
                target: target::PROTOCOL,
                Level::INFO,
                telemetry_event = event::SMOKE,
                message = leaked,
            );
        });

        records.assert_no_forbidden_text(&[leaked]);
    }

    #[test]
    fn retained_tracing_does_not_emit_representative_forbidden_text() {
        let forbidden = [
            "Server=tcp:example.database.windows.net;User ID=alice;Password=secret",
            "password=secret",
            "Bearer secret-token",
            "SELECT * FROM sensitive_table WHERE ssn = '123-45-6789'",
            "customer@example.com",
            "[0xde, 0xad, 0xbe, 0xef]",
            "BEGIN CERTIFICATE",
        ];

        let (_output, records) = test_support::capture(|| {
            test_support::emit_smoke_trace();
        });

        records.assert_no_forbidden_text(&forbidden);
    }
}
