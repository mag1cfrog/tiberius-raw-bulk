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

use crate::{EncryptionLevel, Error};
use std::time::Duration;

pub(crate) mod bulk_load;
pub(crate) mod connection;
pub(crate) mod login;
pub(crate) mod sql_browser;
pub(crate) mod tls;
pub(crate) mod token;

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

    /// Preparation of the final bulk-load packet.
    pub(crate) const BULK_LOAD_FINALIZE_PREPARE: &str = "protocol.bulk_load.finalize.prepare";

    /// Write of the final bulk-load packet.
    pub(crate) const BULK_LOAD_FINALIZE_WRITE: &str = "protocol.bulk_load.finalize.write";

    /// Final bulk-load connection flush.
    pub(crate) const BULK_LOAD_FINALIZE_FLUSH: &str = "protocol.bulk_load.finalize.flush";

    /// Wait for the SQL Server bulk-load result.
    pub(crate) const BULK_LOAD_FINALIZE_RESULT: &str = "protocol.bulk_load.finalize.result";

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

    /// Column metadata token decoded.
    pub(crate) const TOKEN_COL_METADATA: &str = "protocol.token.col_metadata";

    /// Row token decoded.
    pub(crate) const TOKEN_ROW: &str = "protocol.token.row";

    /// NBC row token decoded.
    pub(crate) const TOKEN_NBC_ROW: &str = "protocol.token.nbc_row";

    /// Return value token decoded.
    pub(crate) const TOKEN_RETURN_VALUE: &str = "protocol.token.return_value";

    /// Return status token decoded.
    pub(crate) const TOKEN_RETURN_STATUS: &str = "protocol.token.return_status";

    /// Order token decoded.
    pub(crate) const TOKEN_ORDER: &str = "protocol.token.order";

    /// DONE-family token decoded.
    pub(crate) const TOKEN_DONE: &str = "protocol.token.done";

    /// Error token decoded.
    pub(crate) const TOKEN_ERROR: &str = "protocol.token.error";

    /// Info token decoded.
    pub(crate) const TOKEN_INFO: &str = "protocol.token.info";

    /// Environment change token decoded.
    pub(crate) const TOKEN_ENV_CHANGE: &str = "protocol.token.env_change";

    /// Login acknowledgement token decoded.
    pub(crate) const TOKEN_LOGIN_ACK: &str = "protocol.token.login_ack";

    /// Feature extension acknowledgement token decoded.
    pub(crate) const TOKEN_FEATURE_EXT_ACK: &str = "protocol.token.feature_ext_ack";

    /// SSPI token decoded.
    pub(crate) const TOKEN_SSPI: &str = "protocol.token.sspi";

    /// SQL Browser resolution started.
    pub(crate) const SQL_BROWSER_RESOLUTION_START: &str = "protocol.sql_browser.resolution.start";

    /// SQL Browser resolution completed.
    pub(crate) const SQL_BROWSER_RESOLUTION_COMPLETED: &str =
        "protocol.sql_browser.resolution.completed";

    /// SQL Browser resolution timed out.
    pub(crate) const SQL_BROWSER_RESOLUTION_TIMEOUT: &str =
        "protocol.sql_browser.resolution.timeout";

    /// SQL Browser resolution failed.
    pub(crate) const SQL_BROWSER_RESOLUTION_FAILED: &str = "protocol.sql_browser.resolution.failed";

    /// Smoke event used to validate the observability contract helper.
    pub(crate) const SMOKE: &str = "protocol.smoke";
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
    use super::{
        event, target,
        test_support::{self, CapturedRecordKind},
    };
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
