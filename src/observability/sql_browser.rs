use super::{duration_ms, error_category, event, target};
use crate::Error;
use std::time::Duration;
use tracing::Level;

/// Emits safe SQL Browser resolution start telemetry.
pub(crate) fn emit_resolution_start(
    runtime: &'static str,
    address_family: &'static str,
    sql_browser_port: u16,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::TRACE,
        telemetry_event = event::SQL_BROWSER_RESOLUTION_START,
        phase = "sql_browser",
        operation = "resolve_named_instance",
        status = "started",
        runtime = runtime,
        address_family = address_family,
        sql_browser_port = u64::from(sql_browser_port),
    );
}

/// Emits safe SQL Browser resolution completion telemetry.
pub(crate) fn emit_resolution_completed(
    runtime: &'static str,
    address_family: &'static str,
    sql_browser_port: u16,
    resolved_port: u16,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::TRACE,
        telemetry_event = event::SQL_BROWSER_RESOLUTION_COMPLETED,
        phase = "sql_browser",
        operation = "resolve_named_instance",
        status = "completed",
        runtime = runtime,
        address_family = address_family,
        sql_browser_port = u64::from(sql_browser_port),
        resolved_port = u64::from(resolved_port),
    );
}

/// Emits safe SQL Browser timeout telemetry.
pub(crate) fn emit_resolution_timeout(
    runtime: &'static str,
    address_family: &'static str,
    sql_browser_port: u16,
    timeout: Duration,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::WARN,
        telemetry_event = event::SQL_BROWSER_RESOLUTION_TIMEOUT,
        phase = "sql_browser",
        operation = "resolve_named_instance",
        status = "timeout",
        runtime = runtime,
        address_family = address_family,
        sql_browser_port = u64::from(sql_browser_port),
        timeout_elapsed_ms = duration_ms(timeout),
    );
}

/// Emits safe SQL Browser failure telemetry.
pub(crate) fn emit_resolution_failed(
    runtime: &'static str,
    address_family: &'static str,
    sql_browser_port: u16,
    error: &Error,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::WARN,
        telemetry_event = event::SQL_BROWSER_RESOLUTION_FAILED,
        phase = "sql_browser",
        operation = "resolve_named_instance",
        status = "failed",
        runtime = runtime,
        address_family = address_family,
        sql_browser_port = u64::from(sql_browser_port),
        error_category = error_category(error),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::{event, field, target, test_support};
    use std::borrow::Cow;
    use tracing::Level;

    #[test]
    fn helpers_emit_stable_structured_fields() {
        let (_output, records) = test_support::capture(|| {
            emit_resolution_start("tokio", "ipv4", 1434);
            emit_resolution_completed("tokio", "ipv4", 1434, 1433);
            emit_resolution_timeout("tokio", "ipv4", 1434, Duration::from_millis(1000));
            emit_resolution_failed(
                "tokio",
                "ipv4",
                1434,
                &Error::Conversion(Cow::Borrowed("secret-instance parse failed")),
            );
        });

        let start = records
            .event(event::SQL_BROWSER_RESOLUTION_START)
            .unwrap_or_else(|| panic!("missing start event in {records:?}"));
        assert_eq!(target::PROTOCOL, start.target);
        assert_eq!(Level::TRACE, start.level);
        start.assert_field(field::TELEMETRY_EVENT, event::SQL_BROWSER_RESOLUTION_START);
        start.assert_field("phase", "sql_browser");
        start.assert_field("operation", "resolve_named_instance");
        start.assert_field("status", "started");
        start.assert_field("runtime", "tokio");
        start.assert_field("address_family", "ipv4");
        start.assert_field("sql_browser_port", "1434");

        let completed = records
            .event(event::SQL_BROWSER_RESOLUTION_COMPLETED)
            .unwrap_or_else(|| panic!("missing completed event in {records:?}"));
        completed.assert_field("status", "completed");
        completed.assert_field("resolved_port", "1433");

        let timeout = records
            .event(event::SQL_BROWSER_RESOLUTION_TIMEOUT)
            .unwrap_or_else(|| panic!("missing timeout event in {records:?}"));
        assert_eq!(Level::WARN, timeout.level);
        timeout.assert_field("status", "timeout");
        timeout.assert_field("timeout_elapsed_ms", "1000");

        let failed = records
            .event(event::SQL_BROWSER_RESOLUTION_FAILED)
            .unwrap_or_else(|| panic!("missing failed event in {records:?}"));
        failed.assert_field("status", "failed");
        failed.assert_field("error_category", "conversion");
    }

    #[test]
    fn helpers_do_not_emit_forbidden_text() {
        let forbidden = [
            "secret-instance",
            "sql.example.internal",
            "10.0.0.5:1434",
            "Server=tcp:sql.example.internal\\secret-instance",
            "password=secret",
            "Bearer secret-token",
            "[4, 115, 101, 99]",
            "raw browser response",
        ];

        let (_output, records) = test_support::capture(|| {
            emit_resolution_start("async_std", "ipv6", 1434);
            emit_resolution_completed("async_std", "ipv6", 1434, 51433);
            emit_resolution_timeout("smol", "ipv4", 1434, Duration::from_millis(1000));
            emit_resolution_failed(
                "smol",
                "ipv4",
                1434,
                &Error::Conversion(Cow::Borrowed(
                    "Server=tcp:sql.example.internal\\secret-instance;password=secret",
                )),
            );
        });

        records.assert_no_forbidden_text(&forbidden);
    }

    #[test]
    fn helpers_succeed_without_subscriber() {
        test_support::with_no_subscriber(|| {
            emit_resolution_start("tokio", "ipv4", 1434);
            emit_resolution_completed("tokio", "ipv4", 1434, 1433);
            emit_resolution_timeout("tokio", "ipv4", 1434, Duration::from_millis(1000));
            emit_resolution_failed(
                "tokio",
                "ipv4",
                1434,
                &Error::Conversion(Cow::Borrowed("secret-instance parse failed")),
            );
        });
    }
}
