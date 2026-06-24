use super::{duration_ms, encryption_level_name, error_category, event, span, target};
use crate::{EncryptionLevel, Error};
use std::time::Duration;
use tracing::{Level, Span};

/// Returns the safe TLS backend name for the active feature path.
#[cfg(feature = "rustls")]
pub(crate) fn backend_name() -> &'static str {
    "rustls"
}

/// Returns the safe TLS backend name for the active feature path.
#[cfg(all(not(feature = "rustls"), feature = "native-tls"))]
pub(crate) fn backend_name() -> &'static str {
    "native_tls"
}

/// Returns the safe TLS backend name for the active feature path.
#[cfg(all(
    not(feature = "rustls"),
    not(feature = "native-tls"),
    feature = "vendored-openssl"
))]
pub(crate) fn backend_name() -> &'static str {
    "vendored_openssl"
}

/// Returns the safe TLS backend name when no TLS backend is compiled in.
#[cfg(not(any(
    feature = "rustls",
    feature = "native-tls",
    feature = "vendored-openssl"
)))]
pub(crate) fn backend_name() -> &'static str {
    "none"
}

/// Creates the stable TLS negotiation span.
pub(crate) fn negotiation_span(encryption: EncryptionLevel, tls_backend: &'static str) -> Span {
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

/// Emits the stable TLS negotiation start event.
pub(crate) fn emit_negotiation_start(encryption: EncryptionLevel, tls_backend: &'static str) {
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
pub(crate) fn emit_negotiation_completed(
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
pub(crate) fn emit_negotiation_failed(
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
pub(crate) fn emit_trust_config(
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
pub(crate) fn emit_root_certificates_loaded(
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
pub(crate) fn emit_post_login_downgraded(encryption: EncryptionLevel) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        observability::{event, field, span, test_support},
        EncryptionLevel, Error,
    };
    use std::time::Duration;
    use tracing::Level;

    #[test]
    fn helpers_emit_structured_fields() {
        let (_output, records) = test_support::capture(|| {
            let tls = negotiation_span(EncryptionLevel::Required, "rustls");
            let _tls_entered = tls.enter();

            emit_negotiation_start(EncryptionLevel::Required, "rustls");
            emit_trust_config("rustls", "default", true);
            emit_root_certificates_loaded("rustls", 100, 2);
            emit_negotiation_completed(
                Duration::from_millis(8),
                EncryptionLevel::On,
                "rustls",
                true,
            );
        });

        let tls_span = records
            .span(span::TLS_NEGOTIATION)
            .unwrap_or_else(|| panic!("missing tls span in {records:?}"));
        tls_span.assert_field(field::TELEMETRY_EVENT, event::TLS_NEGOTIATION);
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
    }

    #[test]
    fn helpers_succeed_without_subscriber() {
        test_support::with_no_subscriber(|| {
            let tls = negotiation_span(EncryptionLevel::Required, "native_tls");
            let _tls_entered = tls.enter();

            emit_negotiation_start(EncryptionLevel::Required, "native_tls");
            emit_negotiation_failed(
                Duration::from_millis(2),
                EncryptionLevel::Required,
                "native_tls",
                &Error::Tls("certificate PEM secret".to_string()),
            );
            emit_post_login_downgraded(EncryptionLevel::Off);
        });
    }

    #[test]
    fn helpers_preserve_active_caller_parent_span() {
        let (_output, records) = test_support::capture(|| {
            let caller = tracing::span!(Level::INFO, "caller.request");
            let _caller_entered = caller.enter();

            let tls = negotiation_span(EncryptionLevel::Required, "rustls");
            let _tls_entered = tls.enter();
            emit_negotiation_start(EncryptionLevel::Required, "rustls");
        });

        let tls_span = records
            .span(span::TLS_NEGOTIATION)
            .unwrap_or_else(|| panic!("missing tls span in {records:?}"));
        assert_eq!(Some("caller.request"), tls_span.parent_span_name.as_deref());

        let tls_start = records
            .event(event::TLS_NEGOTIATION_START)
            .unwrap_or_else(|| panic!("missing tls start event in {records:?}"));
        assert_eq!(
            Some(span::TLS_NEGOTIATION),
            tls_start.parent_span_name.as_deref()
        );
    }

    #[test]
    fn helpers_do_not_emit_forbidden_text() {
        let forbidden = [
            "-----BEGIN CERTIFICATE-----",
            "MIICsecretDER",
            "Server=tcp:example.database.windows.net",
            "database-secret-name",
            "app-secret-name",
        ];

        let (_output, records) = test_support::capture(|| {
            let tls = negotiation_span(EncryptionLevel::Required, "rustls");
            let _tls_entered = tls.enter();

            emit_trust_config("rustls", "ca_certificate", true);
            emit_root_certificates_loaded("rustls", 1, 1);
            emit_negotiation_failed(
                Duration::from_millis(3),
                EncryptionLevel::Required,
                "rustls",
                &Error::Tls("-----BEGIN CERTIFICATE----- MIICsecretDER".to_string()),
            );
        });

        records.assert_no_forbidden_text(&forbidden);

        let tls_failed = records
            .event(event::TLS_NEGOTIATION_FAILED)
            .unwrap_or_else(|| panic!("missing tls failed event in {records:?}"));
        tls_failed.assert_field("error_category", "tls");
    }
}
