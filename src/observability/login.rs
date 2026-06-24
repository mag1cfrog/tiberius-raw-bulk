use super::{duration_ms, encryption_level_name, error_category, event, span, target};
use crate::{client::AuthMethod, EncryptionLevel, Error};
use std::time::Duration;
use tracing::{Level, Span};

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

/// Creates the stable login/auth flow span.
pub(crate) fn flow_span(encryption: EncryptionLevel, auth_method: &'static str) -> Span {
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

/// Emits the stable login flow start event.
pub(crate) fn emit_flow_start(encryption: EncryptionLevel, auth_method: &'static str) {
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
pub(crate) fn emit_flow_completed(
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
pub(crate) fn emit_flow_failed(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        observability::{event, field, span, test_support},
        EncryptionLevel, Error,
    };
    use std::{borrow::Cow, time::Duration};
    use tracing::Level;

    #[test]
    fn helpers_emit_structured_fields() {
        let (_output, records) = test_support::capture(|| {
            let login = flow_span(EncryptionLevel::On, "sql_server");
            let _login_entered = login.enter();

            emit_flow_start(EncryptionLevel::On, "sql_server");
            emit_flow_completed(Duration::from_millis(11), EncryptionLevel::On, "sql_server");
        });

        let login_span = records
            .span(span::LOGIN_FLOW)
            .unwrap_or_else(|| panic!("missing login span in {records:?}"));
        login_span.assert_field(field::TELEMETRY_EVENT, event::LOGIN_FLOW);
        login_span.assert_field("phase", "login");
        login_span.assert_field("operation", "authenticate");
        login_span.assert_field("auth_method", "sql_server");

        let login_completed = records
            .event(event::LOGIN_FLOW_COMPLETED)
            .unwrap_or_else(|| panic!("missing login completed event in {records:?}"));
        login_completed.assert_field("login_elapsed_ms", "11");
        login_completed.assert_field("auth_method", "sql_server");
    }

    #[test]
    fn helpers_succeed_without_subscriber() {
        test_support::with_no_subscriber(|| {
            let login = flow_span(EncryptionLevel::Off, "aad_token");
            let _login_entered = login.enter();

            emit_flow_start(EncryptionLevel::Off, "aad_token");
            emit_flow_failed(
                Duration::from_millis(4),
                EncryptionLevel::Off,
                "aad_token",
                &Error::Protocol(Cow::Borrowed("Bearer secret-token")),
            );
        });
    }

    #[test]
    fn helpers_preserve_active_caller_parent_span() {
        let (_output, records) = test_support::capture(|| {
            let caller = tracing::span!(Level::INFO, "caller.request");
            let _caller_entered = caller.enter();

            let login = flow_span(EncryptionLevel::On, "integrated");
            let _login_entered = login.enter();
            emit_flow_start(EncryptionLevel::On, "integrated");
        });

        let login_span = records
            .span(span::LOGIN_FLOW)
            .unwrap_or_else(|| panic!("missing login span in {records:?}"));
        assert_eq!(
            Some("caller.request"),
            login_span.parent_span_name.as_deref()
        );

        let login_start = records
            .event(event::LOGIN_FLOW_START)
            .unwrap_or_else(|| panic!("missing login start event in {records:?}"));
        assert_eq!(
            Some(span::LOGIN_FLOW),
            login_start.parent_span_name.as_deref()
        );
    }

    #[test]
    fn helpers_do_not_emit_forbidden_text() {
        let forbidden = [
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
            let login = flow_span(EncryptionLevel::On, "windows");
            let _login_entered = login.enter();

            emit_flow_failed(
                Duration::from_millis(5),
                EncryptionLevel::On,
                "windows",
                &Error::Protocol(Cow::Borrowed(
                    "DOMAIN alice password=secret Bearer secret-token SSPI_PAYLOAD_BYTES",
                )),
            );
        });

        records.assert_no_forbidden_text(&forbidden);

        let login_failed = records
            .event(event::LOGIN_FLOW_FAILED)
            .unwrap_or_else(|| panic!("missing login failed event in {records:?}"));
        login_failed.assert_field("error_category", "protocol");
    }
}
