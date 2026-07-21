use super::{duration_ms, error_category, event, span, target};
use crate::Error;
use std::time::Duration;
use tracing::{Level, Span};

/// Stable bulk-load request counters safe for tracing.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TraceSummary {
    pub(crate) column_count: u64,
    pub(crate) row_count: Option<u64>,
    pub(crate) packet_count: u64,
    pub(crate) packet_payload_bytes: u64,
    pub(crate) packet_header_bytes: u64,
    pub(crate) max_packet_payload_bytes: u64,
    pub(crate) final_packet_payload_bytes: u64,
    pub(crate) write_packets_call_count: u64,
    pub(crate) max_buffered_bytes_before_write: u64,
    pub(crate) buffered_bytes_after_last_write: u64,
    pub(crate) packet_payload_limit_bytes: u64,
    pub(crate) direct_packet_writes: bool,
}

/// Mutable bulk-load tracing counters.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Telemetry {
    row_count: Option<u64>,
    write_packets_calls: u64,
    packets_written: u64,
    packet_payload_bytes: u64,
    packet_header_bytes: u64,
    max_packet_payload_bytes: u64,
    final_packet_payload_bytes: u64,
    max_buffered_bytes_before_write: u64,
    buffered_bytes_after_last_write: u64,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            row_count: Some(0),
            write_packets_calls: 0,
            packets_written: 0,
            packet_payload_bytes: 0,
            packet_header_bytes: 0,
            max_packet_payload_bytes: 0,
            final_packet_payload_bytes: 0,
            max_buffered_bytes_before_write: 0,
            buffered_bytes_after_last_write: 0,
        }
    }
}

impl Telemetry {
    /// Records rows from a source that safely knows row count.
    pub(crate) fn record_known_rows(&mut self, row_count: u64) {
        if let Some(known_rows) = &mut self.row_count {
            *known_rows = known_rows.saturating_add(row_count);
        }
    }

    /// Marks row count unknown for raw APIs that do not safely know it.
    pub(crate) fn mark_row_count_unknown(&mut self) {
        self.row_count = None;
    }

    /// Records one bulk packet drain attempt.
    pub(crate) fn record_write_packets_call(&mut self, buffered_bytes_before_write: u64) {
        self.write_packets_calls = self.write_packets_calls.saturating_add(1);
        self.max_buffered_bytes_before_write = self
            .max_buffered_bytes_before_write
            .max(buffered_bytes_before_write);
    }

    /// Records the buffered tail after a packet drain attempt.
    pub(crate) fn record_buffered_bytes_after_write(&mut self, buffered_bytes_after_write: u64) {
        self.buffered_bytes_after_last_write = buffered_bytes_after_write;
    }

    /// Records one successfully written bulk packet.
    pub(crate) fn record_packet_written(
        &mut self,
        packet_payload_bytes: u64,
        packet_header_bytes: u64,
        final_packet: bool,
    ) {
        self.packets_written = self.packets_written.saturating_add(1);
        self.packet_payload_bytes = self
            .packet_payload_bytes
            .saturating_add(packet_payload_bytes);
        self.packet_header_bytes = self.packet_header_bytes.saturating_add(packet_header_bytes);
        self.max_packet_payload_bytes = self.max_packet_payload_bytes.max(packet_payload_bytes);

        if final_packet {
            self.final_packet_payload_bytes = packet_payload_bytes;
        }
    }

    /// Builds a stable summary for request completion or failure events.
    pub(crate) fn summary(
        self,
        column_count: u64,
        packet_payload_limit_bytes: u64,
        direct_packet_writes: bool,
    ) -> TraceSummary {
        TraceSummary {
            column_count,
            row_count: self.row_count,
            packet_count: self.packets_written,
            packet_payload_bytes: self.packet_payload_bytes,
            packet_header_bytes: self.packet_header_bytes,
            max_packet_payload_bytes: self.max_packet_payload_bytes,
            final_packet_payload_bytes: self.final_packet_payload_bytes,
            write_packets_call_count: self.write_packets_calls,
            max_buffered_bytes_before_write: self.max_buffered_bytes_before_write,
            buffered_bytes_after_last_write: self.buffered_bytes_after_last_write,
            packet_payload_limit_bytes,
            direct_packet_writes,
        }
    }

    #[cfg(test)]
    fn row_count(&self) -> Option<u64> {
        self.row_count
    }

    #[cfg(test)]
    fn write_packets_calls(&self) -> u64 {
        self.write_packets_calls
    }
}

/// Per-request bulk-load tracing state and emit helpers.
#[derive(Debug)]
pub(crate) struct RequestTrace {
    span: Span,
    telemetry: Telemetry,
    column_count: u64,
    packet_payload_limit_bytes: u64,
    direct_packet_writes: bool,
}

impl RequestTrace {
    /// Creates a bulk-load request trace and emits the request start event.
    pub(crate) fn new(column_count: u64, packet_payload_limit_bytes: u64) -> Self {
        let span = request_span(column_count, packet_payload_limit_bytes);
        let trace = Self {
            span,
            telemetry: Telemetry::default(),
            column_count,
            packet_payload_limit_bytes,
            direct_packet_writes: false,
        };

        trace.span.in_scope(|| {
            emit_request_start(column_count, packet_payload_limit_bytes, false);
        });

        trace
    }

    /// Records that the request switched to direct packet writes.
    pub(crate) fn set_direct_packet_writes(&mut self, direct_packet_writes: bool) {
        self.direct_packet_writes = direct_packet_writes;
    }

    /// Records rows from a source that safely knows row count.
    pub(crate) fn record_known_rows(&mut self, row_count: u64) {
        self.telemetry.record_known_rows(row_count);
    }

    /// Marks row count unknown for raw APIs that do not safely know it.
    pub(crate) fn mark_row_count_unknown(&mut self) {
        self.telemetry.mark_row_count_unknown();
    }

    /// Records one bulk packet drain attempt.
    pub(crate) fn record_write_packets_call(&mut self, buffered_bytes_before_write: u64) {
        self.telemetry
            .record_write_packets_call(buffered_bytes_before_write);
    }

    /// Records the buffered tail after a packet drain attempt.
    pub(crate) fn record_buffered_bytes_after_write(&mut self, buffered_bytes_after_write: u64) {
        self.telemetry
            .record_buffered_bytes_after_write(buffered_bytes_after_write);
    }

    /// Records and emits one successfully written bulk packet summary.
    pub(crate) fn record_packet_written(
        &mut self,
        packet_payload_bytes: u64,
        packet_header_bytes: u64,
        final_packet: bool,
    ) {
        self.telemetry.record_packet_written(
            packet_payload_bytes,
            packet_header_bytes,
            final_packet,
        );

        self.span.in_scope(|| {
            emit_packet_written(
                packet_payload_bytes,
                packet_header_bytes,
                self.packet_payload_limit_bytes,
                self.direct_packet_writes,
                final_packet,
            );
        });
    }

    /// Emits the stable bulk-load flush completed event.
    pub(crate) fn emit_flush_completed(&self, flush_elapsed: Duration) {
        let summary = self.summary();

        self.span.in_scope(|| {
            emit_flush_completed(
                flush_elapsed,
                summary.packet_count,
                summary.packet_payload_bytes,
                summary.packet_header_bytes,
                self.direct_packet_writes,
            );
        });
    }

    /// Emits the stable bulk-load flush failed event.
    pub(crate) fn emit_flush_failed(&self, flush_elapsed: Duration, error: &Error) {
        let summary = self.summary();

        self.span.in_scope(|| {
            emit_flush_failed(
                flush_elapsed,
                summary.packet_count,
                summary.packet_payload_bytes,
                summary.packet_header_bytes,
                self.direct_packet_writes,
                error,
            );
        });
    }

    /// Emits the stable bulk-load request completed event.
    pub(crate) fn emit_request_completed(&self, request_elapsed: Duration) {
        self.span.in_scope(|| {
            emit_request_completed(request_elapsed, self.summary());
        });
    }

    /// Emits the stable bulk-load request failed event.
    pub(crate) fn emit_request_failed(&self, request_elapsed: Duration, error: &Error) {
        self.span.in_scope(|| {
            emit_request_failed(request_elapsed, self.summary(), error);
        });
    }

    fn summary(&self) -> TraceSummary {
        self.telemetry.summary(
            self.column_count,
            self.packet_payload_limit_bytes,
            self.direct_packet_writes,
        )
    }
}

/// Creates the stable bulk-load request span.
pub(crate) fn request_span(column_count: u64, packet_payload_limit_bytes: u64) -> Span {
    tracing::span!(
        target: target::PROTOCOL,
        Level::INFO,
        span::BULK_LOAD_REQUEST,
        telemetry_event = event::BULK_LOAD_REQUEST,
        phase = "bulk_load",
        operation = "request",
        column_count = column_count,
        packet_payload_limit_bytes = packet_payload_limit_bytes,
    )
}

/// Creates the span that prepares the final bulk-load packet.
pub(crate) fn finalize_prepare_span() -> Span {
    tracing::span!(
        target: target::PROTOCOL,
        Level::INFO,
        span::BULK_LOAD_FINALIZE_PREPARE,
        phase = "bulk_load",
        operation = "finalize_prepare",
    )
}

/// Creates the span that writes the final bulk-load packet.
pub(crate) fn finalize_write_span() -> Span {
    tracing::span!(
        target: target::PROTOCOL,
        Level::INFO,
        span::BULK_LOAD_FINALIZE_WRITE,
        phase = "bulk_load",
        operation = "finalize_write",
    )
}

/// Creates the span that flushes the finalized bulk-load request.
pub(crate) fn finalize_flush_span() -> Span {
    tracing::span!(
        target: target::PROTOCOL,
        Level::INFO,
        span::BULK_LOAD_FINALIZE_FLUSH,
        phase = "bulk_load",
        operation = "finalize_flush",
    )
}

/// Creates the span that waits for the SQL Server bulk-load result.
pub(crate) fn finalize_result_span() -> Span {
    tracing::span!(
        target: target::PROTOCOL,
        Level::INFO,
        span::BULK_LOAD_FINALIZE_RESULT,
        phase = "bulk_load",
        operation = "finalize_result",
    )
}

/// Emits the stable bulk-load request start event.
pub(crate) fn emit_request_start(
    column_count: u64,
    packet_payload_limit_bytes: u64,
    direct_packet_writes: bool,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::BULK_LOAD_REQUEST_START,
        phase = "bulk_load",
        operation = "request",
        status = "started",
        column_count = column_count,
        packet_payload_limit_bytes = packet_payload_limit_bytes,
        protocol_path = protocol_path(direct_packet_writes),
        direct_packet_writes = direct_packet_writes,
    );
}

/// Emits the stable bulk-load request completed event.
pub(crate) fn emit_request_completed(request_elapsed: Duration, summary: TraceSummary) {
    match summary.row_count {
        Some(row_count) => tracing::event!(
            target: target::PROTOCOL,
            Level::INFO,
            telemetry_event = event::BULK_LOAD_REQUEST_COMPLETED,
            phase = "bulk_load",
            operation = "request",
            status = "completed",
            request_elapsed_ms = duration_ms(request_elapsed),
            column_count = summary.column_count,
            row_count_known = true,
            row_count = row_count,
            packet_count = summary.packet_count,
            packet_payload_bytes = summary.packet_payload_bytes,
            packet_header_bytes = summary.packet_header_bytes,
            max_packet_payload_bytes = summary.max_packet_payload_bytes,
            final_packet_payload_bytes = summary.final_packet_payload_bytes,
            write_packets_call_count = summary.write_packets_call_count,
            max_buffered_bytes_before_write = summary.max_buffered_bytes_before_write,
            buffered_bytes_after_last_write = summary.buffered_bytes_after_last_write,
            packet_payload_limit_bytes = summary.packet_payload_limit_bytes,
            protocol_path = protocol_path(summary.direct_packet_writes),
            direct_packet_writes = summary.direct_packet_writes,
        ),
        None => tracing::event!(
            target: target::PROTOCOL,
            Level::INFO,
            telemetry_event = event::BULK_LOAD_REQUEST_COMPLETED,
            phase = "bulk_load",
            operation = "request",
            status = "completed",
            request_elapsed_ms = duration_ms(request_elapsed),
            column_count = summary.column_count,
            row_count_known = false,
            packet_count = summary.packet_count,
            packet_payload_bytes = summary.packet_payload_bytes,
            packet_header_bytes = summary.packet_header_bytes,
            max_packet_payload_bytes = summary.max_packet_payload_bytes,
            final_packet_payload_bytes = summary.final_packet_payload_bytes,
            write_packets_call_count = summary.write_packets_call_count,
            max_buffered_bytes_before_write = summary.max_buffered_bytes_before_write,
            buffered_bytes_after_last_write = summary.buffered_bytes_after_last_write,
            packet_payload_limit_bytes = summary.packet_payload_limit_bytes,
            protocol_path = protocol_path(summary.direct_packet_writes),
            direct_packet_writes = summary.direct_packet_writes,
        ),
    }
}

/// Emits the stable bulk-load request failed event.
pub(crate) fn emit_request_failed(request_elapsed: Duration, summary: TraceSummary, error: &Error) {
    match summary.row_count {
        Some(row_count) => tracing::event!(
            target: target::PROTOCOL,
            Level::WARN,
            telemetry_event = event::BULK_LOAD_REQUEST_FAILED,
            phase = "bulk_load",
            operation = "request",
            status = "failed",
            request_elapsed_ms = duration_ms(request_elapsed),
            column_count = summary.column_count,
            row_count_known = true,
            row_count = row_count,
            packet_count = summary.packet_count,
            packet_payload_bytes = summary.packet_payload_bytes,
            packet_header_bytes = summary.packet_header_bytes,
            max_packet_payload_bytes = summary.max_packet_payload_bytes,
            final_packet_payload_bytes = summary.final_packet_payload_bytes,
            write_packets_call_count = summary.write_packets_call_count,
            max_buffered_bytes_before_write = summary.max_buffered_bytes_before_write,
            buffered_bytes_after_last_write = summary.buffered_bytes_after_last_write,
            packet_payload_limit_bytes = summary.packet_payload_limit_bytes,
            protocol_path = protocol_path(summary.direct_packet_writes),
            direct_packet_writes = summary.direct_packet_writes,
            error_category = error_category(error),
        ),
        None => tracing::event!(
            target: target::PROTOCOL,
            Level::WARN,
            telemetry_event = event::BULK_LOAD_REQUEST_FAILED,
            phase = "bulk_load",
            operation = "request",
            status = "failed",
            request_elapsed_ms = duration_ms(request_elapsed),
            column_count = summary.column_count,
            row_count_known = false,
            packet_count = summary.packet_count,
            packet_payload_bytes = summary.packet_payload_bytes,
            packet_header_bytes = summary.packet_header_bytes,
            max_packet_payload_bytes = summary.max_packet_payload_bytes,
            final_packet_payload_bytes = summary.final_packet_payload_bytes,
            write_packets_call_count = summary.write_packets_call_count,
            max_buffered_bytes_before_write = summary.max_buffered_bytes_before_write,
            buffered_bytes_after_last_write = summary.buffered_bytes_after_last_write,
            packet_payload_limit_bytes = summary.packet_payload_limit_bytes,
            protocol_path = protocol_path(summary.direct_packet_writes),
            direct_packet_writes = summary.direct_packet_writes,
            error_category = error_category(error),
        ),
    }
}

/// Emits the stable bulk-load packet write summary event.
pub(crate) fn emit_packet_written(
    packet_payload_bytes: u64,
    packet_header_bytes: u64,
    packet_payload_limit_bytes: u64,
    direct_packet_writes: bool,
    final_packet: bool,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::BULK_LOAD_PACKET_WRITTEN,
        phase = "bulk_load",
        operation = "write_packet",
        status = "completed",
        packet_payload_bytes = packet_payload_bytes,
        packet_header_bytes = packet_header_bytes,
        packet_bytes = packet_payload_bytes.saturating_add(packet_header_bytes),
        packet_payload_limit_bytes = packet_payload_limit_bytes,
        protocol_path = protocol_path(direct_packet_writes),
        direct_packet_writes = direct_packet_writes,
        final_packet = final_packet,
    );
}

/// Emits the stable bulk-load flush completed event.
pub(crate) fn emit_flush_completed(
    flush_elapsed: Duration,
    packet_count: u64,
    packet_payload_bytes: u64,
    packet_header_bytes: u64,
    direct_packet_writes: bool,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::BULK_LOAD_FLUSH_COMPLETED,
        phase = "bulk_load",
        operation = "flush",
        status = "completed",
        flush_elapsed_ms = duration_ms(flush_elapsed),
        packet_count = packet_count,
        packet_payload_bytes = packet_payload_bytes,
        packet_header_bytes = packet_header_bytes,
        protocol_path = protocol_path(direct_packet_writes),
        direct_packet_writes = direct_packet_writes,
    );
}

/// Emits the stable bulk-load flush failed event.
pub(crate) fn emit_flush_failed(
    flush_elapsed: Duration,
    packet_count: u64,
    packet_payload_bytes: u64,
    packet_header_bytes: u64,
    direct_packet_writes: bool,
    error: &Error,
) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::WARN,
        telemetry_event = event::BULK_LOAD_FLUSH_FAILED,
        phase = "bulk_load",
        operation = "flush",
        status = "failed",
        flush_elapsed_ms = duration_ms(flush_elapsed),
        packet_count = packet_count,
        packet_payload_bytes = packet_payload_bytes,
        packet_header_bytes = packet_header_bytes,
        protocol_path = protocol_path(direct_packet_writes),
        direct_packet_writes = direct_packet_writes,
        error_category = error_category(error),
    );
}

fn protocol_path(direct_packet_writes: bool) -> &'static str {
    if direct_packet_writes {
        "direct_packet"
    } else {
        "framed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        observability::{event, field, span, target, test_support},
        Error,
    };
    use std::{borrow::Cow, time::Duration};
    use tracing::Level;

    #[test]
    fn helpers_emit_structured_fields() {
        let (_output, records) = test_support::capture(|| {
            let bulk_load = request_span(3, 4096);
            let _entered = bulk_load.enter();

            emit_request_start(3, 4096, false);
            emit_packet_written(1024, 8, 4096, false, false);
            emit_flush_completed(Duration::from_millis(7), 2, 2048, 16, false);
            emit_request_completed(
                Duration::from_millis(11),
                TraceSummary {
                    column_count: 3,
                    row_count: Some(2),
                    packet_count: 3,
                    packet_payload_bytes: 2080,
                    packet_header_bytes: 24,
                    max_packet_payload_bytes: 1024,
                    final_packet_payload_bytes: 32,
                    write_packets_call_count: 5,
                    max_buffered_bytes_before_write: 8192,
                    buffered_bytes_after_last_write: 64,
                    packet_payload_limit_bytes: 4096,
                    direct_packet_writes: false,
                },
            );
        });

        let bulk_load_span = records
            .span(span::BULK_LOAD_REQUEST)
            .unwrap_or_else(|| panic!("missing bulk-load span in {records:?}"));
        bulk_load_span.assert_field(field::TELEMETRY_EVENT, event::BULK_LOAD_REQUEST);
        bulk_load_span.assert_field("phase", "bulk_load");
        bulk_load_span.assert_field("operation", "request");
        bulk_load_span.assert_field("column_count", "3");
        bulk_load_span.assert_field("packet_payload_limit_bytes", "4096");

        let packet = records
            .event(event::BULK_LOAD_PACKET_WRITTEN)
            .unwrap_or_else(|| panic!("missing bulk-load packet event in {records:?}"));
        packet.assert_field("packet_payload_bytes", "1024");
        packet.assert_field("packet_header_bytes", "8");
        packet.assert_field("packet_bytes", "1032");
        packet.assert_field("protocol_path", "framed");
        packet.assert_field("direct_packet_writes", "false");
        packet.assert_field("final_packet", "false");

        let flush = records
            .event(event::BULK_LOAD_FLUSH_COMPLETED)
            .unwrap_or_else(|| panic!("missing bulk-load flush event in {records:?}"));
        flush.assert_field("flush_elapsed_ms", "7");
        flush.assert_field("packet_count", "2");
        flush.assert_field("packet_payload_bytes", "2048");

        let completed = records
            .event(event::BULK_LOAD_REQUEST_COMPLETED)
            .unwrap_or_else(|| panic!("missing bulk-load completed event in {records:?}"));
        completed.assert_field("request_elapsed_ms", "11");
        completed.assert_field("row_count_known", "true");
        completed.assert_field("row_count", "2");
        completed.assert_field("packet_count", "3");
        completed.assert_field("packet_payload_bytes", "2080");
        completed.assert_field("packet_header_bytes", "24");
        completed.assert_field("max_packet_payload_bytes", "1024");
        completed.assert_field("final_packet_payload_bytes", "32");
        completed.assert_field("write_packets_call_count", "5");
        completed.assert_field("max_buffered_bytes_before_write", "8192");
        completed.assert_field("buffered_bytes_after_last_write", "64");
    }

    #[test]
    fn helpers_support_unknown_row_count() {
        let (_output, records) = test_support::capture(|| {
            let bulk_load = request_span(2, 512);
            let _entered = bulk_load.enter();

            emit_request_completed(
                Duration::from_millis(1),
                TraceSummary {
                    column_count: 2,
                    row_count: None,
                    packet_count: 1,
                    packet_payload_bytes: 20,
                    packet_header_bytes: 8,
                    max_packet_payload_bytes: 20,
                    final_packet_payload_bytes: 20,
                    write_packets_call_count: 1,
                    max_buffered_bytes_before_write: 20,
                    buffered_bytes_after_last_write: 20,
                    packet_payload_limit_bytes: 512,
                    direct_packet_writes: true,
                },
            );
        });

        let completed = records
            .event(event::BULK_LOAD_REQUEST_COMPLETED)
            .unwrap_or_else(|| panic!("missing bulk-load completed event in {records:?}"));
        completed.assert_field("row_count_known", "false");
        assert_eq!(None, completed.field("row_count"));
        completed.assert_field("protocol_path", "direct_packet");
        completed.assert_field("direct_packet_writes", "true");
    }

    #[test]
    fn helpers_succeed_without_subscriber() {
        test_support::with_no_subscriber(|| {
            let bulk_load = request_span(1, 4096);
            let _entered = bulk_load.enter();

            emit_request_start(1, 4096, false);
            emit_packet_written(10, 8, 4096, false, true);
            emit_flush_failed(
                Duration::from_millis(2),
                1,
                10,
                8,
                false,
                &Error::Io {
                    kind: crate::error::IoErrorKind::ConnectionAborted,
                    message: "row payload secret".into(),
                },
            );
            emit_request_failed(
                Duration::from_millis(3),
                TraceSummary {
                    column_count: 1,
                    row_count: Some(1),
                    packet_count: 1,
                    packet_payload_bytes: 10,
                    packet_header_bytes: 8,
                    max_packet_payload_bytes: 10,
                    final_packet_payload_bytes: 10,
                    write_packets_call_count: 1,
                    max_buffered_bytes_before_write: 10,
                    buffered_bytes_after_last_write: 10,
                    packet_payload_limit_bytes: 4096,
                    direct_packet_writes: false,
                },
                &Error::BulkInput(Cow::Borrowed("raw row value secret")),
            );
        });
    }

    #[test]
    fn helpers_preserve_active_caller_parent_span() {
        let (_output, records) = test_support::capture(|| {
            let caller = tracing::span!(Level::INFO, "caller.request");
            let _caller_entered = caller.enter();

            let bulk_load = request_span(1, 4096);
            let _bulk_load_entered = bulk_load.enter();

            emit_request_start(1, 4096, false);
            emit_packet_written(12, 8, 4096, false, false);
        });

        let bulk_load_span = records
            .span(span::BULK_LOAD_REQUEST)
            .unwrap_or_else(|| panic!("missing bulk-load span in {records:?}"));
        assert_eq!(
            Some("caller.request"),
            bulk_load_span.parent_span_name.as_deref()
        );

        let start = records
            .event(event::BULK_LOAD_REQUEST_START)
            .unwrap_or_else(|| panic!("missing bulk-load start event in {records:?}"));
        assert_eq!(
            Some(span::BULK_LOAD_REQUEST),
            start.parent_span_name.as_deref()
        );

        let packet = records
            .event(event::BULK_LOAD_PACKET_WRITTEN)
            .unwrap_or_else(|| panic!("missing bulk-load packet event in {records:?}"));
        assert_eq!(
            Some(span::BULK_LOAD_REQUEST),
            packet.parent_span_name.as_deref()
        );
    }

    #[test]
    fn finalization_spans_preserve_the_active_caller() {
        let (_output, records) = test_support::capture(|| {
            let caller = tracing::span!(Level::INFO, "caller.finalize");
            let _caller_entered = caller.enter();

            drop(finalize_prepare_span());
            drop(finalize_write_span());
            drop(finalize_flush_span());
            drop(finalize_result_span());
        });

        for (name, operation) in [
            (span::BULK_LOAD_FINALIZE_PREPARE, "finalize_prepare"),
            (span::BULK_LOAD_FINALIZE_WRITE, "finalize_write"),
            (span::BULK_LOAD_FINALIZE_FLUSH, "finalize_flush"),
            (span::BULK_LOAD_FINALIZE_RESULT, "finalize_result"),
        ] {
            let captured = records
                .span(name)
                .unwrap_or_else(|| panic!("missing finalization span `{name}` in {records:?}"));
            assert_eq!(target::PROTOCOL, captured.target);
            assert_eq!(Level::INFO, captured.level);
            assert_eq!(
                Some("caller.finalize"),
                captured.parent_span_name.as_deref()
            );
            captured.assert_field("phase", "bulk_load");
            captured.assert_field("operation", operation);
        }
    }

    #[test]
    fn helpers_do_not_emit_forbidden_text() {
        let forbidden = [
            "INSERT BULK secret_table",
            "secret_table",
            "raw-row-value",
            "raw-payload-bytes",
            "Server=tcp:example.database.windows.net",
            "password=secret",
            "server returned arbitrary text",
        ];

        let (_output, records) = test_support::capture(|| {
            let bulk_load = request_span(4, 4096);
            let _entered = bulk_load.enter();

            emit_request_start(4, 4096, true);
            emit_packet_written(128, 8, 4096, true, true);
            emit_request_failed(
                Duration::from_millis(13),
                TraceSummary {
                    column_count: 4,
                    row_count: None,
                    packet_count: 1,
                    packet_payload_bytes: 128,
                    packet_header_bytes: 8,
                    max_packet_payload_bytes: 128,
                    final_packet_payload_bytes: 128,
                    write_packets_call_count: 1,
                    max_buffered_bytes_before_write: 128,
                    buffered_bytes_after_last_write: 128,
                    packet_payload_limit_bytes: 4096,
                    direct_packet_writes: true,
                },
                &Error::Protocol(Cow::Borrowed(
                    "INSERT BULK secret_table raw-row-value raw-payload-bytes",
                )),
            );
        });

        records.assert_no_forbidden_text(&forbidden);

        let failed = records
            .event(event::BULK_LOAD_REQUEST_FAILED)
            .unwrap_or_else(|| panic!("missing bulk-load failed event in {records:?}"));
        failed.assert_field("error_category", "protocol");
        failed.assert_field("row_count_known", "false");
    }

    #[test]
    fn telemetry_tracks_known_and_unknown_rows() {
        let mut telemetry = Telemetry::default();

        assert_eq!(Some(0), telemetry.row_count());

        telemetry.record_known_rows(1);
        telemetry.record_known_rows(3);
        assert_eq!(Some(4), telemetry.row_count());

        telemetry.mark_row_count_unknown();
        telemetry.record_known_rows(10);
        assert_eq!(None, telemetry.row_count());
    }

    #[test]
    fn telemetry_tracks_packet_and_buffer_summaries() {
        let mut telemetry = Telemetry::default();

        telemetry.record_write_packets_call(128);
        telemetry.record_packet_written(40, 8, false);
        telemetry.record_packet_written(7, 8, true);
        telemetry.record_buffered_bytes_after_write(3);

        assert_eq!(1, telemetry.write_packets_calls());

        let summary = telemetry.summary(2, 4096, true);
        assert_eq!(2, summary.column_count);
        assert_eq!(2, summary.packet_count);
        assert_eq!(47, summary.packet_payload_bytes);
        assert_eq!(16, summary.packet_header_bytes);
        assert_eq!(40, summary.max_packet_payload_bytes);
        assert_eq!(7, summary.final_packet_payload_bytes);
        assert_eq!(128, summary.max_buffered_bytes_before_write);
        assert_eq!(3, summary.buffered_bytes_after_last_write);
        assert_eq!(4096, summary.packet_payload_limit_bytes);
        assert!(summary.direct_packet_writes);
    }
}
