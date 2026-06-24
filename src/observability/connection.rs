use super::{duration_ms, encryption_level_name, error_category, event, span, target};
use crate::{EncryptionLevel, Error};
use std::time::Duration;
use tracing::{Level, Span};

/// Creates the stable span for connection setup on an already-open transport.
pub(crate) fn connect_span(requested_encryption: EncryptionLevel, fed_auth_required: bool) -> Span {
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

/// Emits the stable connection setup start event.
pub(crate) fn emit_setup_start(requested_encryption: EncryptionLevel, fed_auth_required: bool) {
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
pub(crate) fn emit_setup_completed(
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
pub(crate) fn emit_setup_failed(setup_elapsed: Duration, error: &Error) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        observability::{event, field, span, target, test_support},
        EncryptionLevel, Error,
    };
    use std::{borrow::Cow, time::Duration};
    use tracing::Level;

    #[test]
    fn helpers_emit_structured_fields() {
        let (_output, records) = test_support::capture(|| {
            let connection = connect_span(EncryptionLevel::Required, true);
            let _entered = connection.enter();

            emit_setup_start(EncryptionLevel::Required, true);
            emit_prelogin_start(EncryptionLevel::Required, true);
            emit_prelogin_completed(Duration::from_millis(4), EncryptionLevel::On, true);
            emit_setup_completed(Duration::from_millis(9), EncryptionLevel::On, 4096);
        });

        let connection_span = records
            .span(span::CONNECTION_CONNECT)
            .unwrap_or_else(|| panic!("missing connection span in {records:?}"));
        assert_eq!(target::PROTOCOL, connection_span.target);
        assert_eq!(Level::INFO, connection_span.level);
        connection_span.assert_field(field::TELEMETRY_EVENT, event::CONNECTION_CONNECT);
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
    fn helpers_succeed_without_subscriber() {
        test_support::with_no_subscriber(|| {
            let connection = connect_span(EncryptionLevel::Off, false);
            let _entered = connection.enter();

            emit_setup_start(EncryptionLevel::Off, false);
            emit_prelogin_start(EncryptionLevel::Off, false);
            emit_prelogin_failed(
                Duration::from_millis(1),
                &Error::Protocol(Cow::Borrowed("protocol failure")),
            );
            emit_setup_failed(
                Duration::from_millis(2),
                &Error::Protocol(Cow::Borrowed("protocol failure")),
            );
        });
    }

    #[test]
    fn helpers_preserve_active_caller_parent_span() {
        let (_output, records) = test_support::capture(|| {
            let caller = tracing::span!(Level::INFO, "caller.request");
            let _caller_entered = caller.enter();

            let connection = connect_span(EncryptionLevel::Required, false);
            let _connection_entered = connection.enter();

            emit_prelogin_start(EncryptionLevel::Required, false);
        });

        let connection_span = records
            .span(span::CONNECTION_CONNECT)
            .unwrap_or_else(|| panic!("missing connection span in {records:?}"));
        assert_eq!(
            Some("caller.request"),
            connection_span.parent_span_name.as_deref()
        );

        let prelogin_start = records
            .event(event::CONNECTION_PRELOGIN_START)
            .unwrap_or_else(|| panic!("missing prelogin start event in {records:?}"));
        assert_eq!(
            Some(span::CONNECTION_CONNECT),
            prelogin_start.parent_span_name.as_deref()
        );
    }

    #[test]
    fn helpers_do_not_emit_forbidden_text() {
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
            let connection = connect_span(EncryptionLevel::Required, true);
            let _entered = connection.enter();

            emit_setup_start(EncryptionLevel::Required, true);
            emit_prelogin_start(EncryptionLevel::Required, true);
            emit_prelogin_failed(
                Duration::from_millis(3),
                &Error::Protocol(Cow::Borrowed(
                    "Server=tcp:example.database.windows.net;User ID=alice;Password=secret",
                )),
            );
            emit_setup_failed(
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
}
