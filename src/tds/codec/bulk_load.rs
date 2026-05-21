use asynchronous_codec::BytesMut;
use bytes::BufMut;
use enumflags2::BitFlags;
use futures_util::io::{AsyncRead, AsyncWrite};
use std::time::{Duration, Instant};
use tracing::{event, Level};

use crate::{
    client::{Connection, DirectPacketWriteTiming},
    sql_read_bytes::SqlReadBytes,
    BytesMutWithDataColumns, ColumnFlag, ColumnType, ExecuteResult,
};

use super::{
    Encode, MetaDataColumn, PacketHeader, PacketStatus, TokenColMetaData, TokenDone, TokenRow,
    TokenType, TypeInfo, HEADER_BYTES,
};

/// Owned destination metadata for a bulk-load request.
#[derive(Debug, Clone)]
pub struct BulkLoadColumns<'a> {
    columns: Vec<MetaDataColumn<'a>>,
}

impl<'a> BulkLoadColumns<'a> {
    pub(crate) fn new(columns: Vec<MetaDataColumn<'a>>) -> Self {
        Self { columns }
    }

    pub(crate) fn into_inner(self) -> Vec<MetaDataColumn<'a>> {
        self.columns
    }

    /// The number of destination columns in the bulk row encoding order.
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Whether there are no destination columns.
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Returns the destination columns in bulk row encoding order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = BulkLoadColumn<'_>> {
        bulk_load_columns(&self.columns)
    }
}

/// Metadata for rows appended directly into a raw bulk-load buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRowsAppend {
    row_token_offsets: Vec<usize>,
}

impl RawRowsAppend {
    /// Creates appended-row metadata from row-token offsets.
    ///
    /// Offsets must be relative to the start of the appended byte region, not
    /// to the start of the full bulk-load request buffer.
    pub fn new(row_token_offsets: Vec<usize>) -> Self {
        Self { row_token_offsets }
    }

    /// Returns row-token offsets relative to the appended byte region.
    pub fn row_token_offsets(&self) -> &[usize] {
        &self.row_token_offsets
    }
}

/// Append-only access to a raw bulk-load request buffer.
///
/// This is a capability wrapper for [`BulkLoadRequest::send_raw_rows_with`].
/// It lets callers append encoded row bytes directly into the request buffer
/// without exposing `BytesMut` operations that could truncate, split, clear, or
/// otherwise mutate bytes that existed before the append started. That keeps
/// the method's rollback behavior well-defined when encoding or validation
/// fails.
#[derive(Debug)]
pub struct RawRowsAppendBuffer<'a> {
    bytes: &'a mut BytesMut,
}

impl RawRowsAppendBuffer<'_> {
    /// Appends raw row bytes to the request buffer.
    pub fn extend_from_slice(&mut self, slice: &[u8]) {
        self.bytes.extend_from_slice(slice);
    }

    /// Appends one raw row byte to the request buffer.
    pub fn put_u8(&mut self, value: u8) {
        self.bytes.put_u8(value);
    }

    /// Appends a little-endian 16-bit unsigned integer.
    pub fn put_u16_le(&mut self, value: u16) {
        self.bytes.put_u16_le(value);
    }

    /// Appends a little-endian 32-bit unsigned integer.
    pub fn put_u32_le(&mut self, value: u32) {
        self.bytes.put_u32_le(value);
    }

    /// Appends a little-endian 64-bit unsigned integer.
    pub fn put_u64_le(&mut self, value: u64) {
        self.bytes.put_u64_le(value);
    }
}

/// Packet-write statistics collected by a bulk-load request.
///
/// These counters are intended for benchmarking and diagnostics. They do not
/// change bulk-load behavior and do not include TDS packet header bytes unless
/// a field name explicitly says otherwise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BulkLoadPacketStats {
    /// Number of times the request attempted to drain complete packets.
    pub write_packets_calls: u64,
    /// Number of complete bulk-load packets written before finalization.
    pub packets_written: u64,
    /// Complete packet payload bytes written before finalization.
    pub packet_payload_bytes: u64,
    /// Largest complete packet payload written before finalization.
    pub max_packet_payload_bytes: usize,
    /// Largest buffered byte count observed before draining packets.
    pub max_buffered_bytes_before_write: usize,
    /// Buffered tail bytes left after the most recent packet drain.
    pub buffered_bytes_after_last_write: usize,
    /// Payload bytes written by the final `EndOfMessage` packet.
    pub finalized_packet_payload_bytes: usize,
}

impl BulkLoadPacketStats {
    fn record_write_packets_call(&mut self, buffered_bytes_before_write: usize) {
        self.write_packets_calls = self.write_packets_calls.saturating_add(1);
        self.max_buffered_bytes_before_write = self
            .max_buffered_bytes_before_write
            .max(buffered_bytes_before_write);
    }

    fn record_packet_written(&mut self, packet_payload_bytes: usize) {
        self.packets_written = self.packets_written.saturating_add(1);
        self.packet_payload_bytes = self
            .packet_payload_bytes
            .saturating_add(usize_to_u64_saturating(packet_payload_bytes));
        self.max_packet_payload_bytes = self.max_packet_payload_bytes.max(packet_payload_bytes);
    }

    fn record_buffered_bytes_after_write(&mut self, buffered_bytes_after_write: usize) {
        self.buffered_bytes_after_last_write = buffered_bytes_after_write;
    }

    fn record_finalized_packet(&mut self, packet_payload_bytes: usize) {
        self.finalized_packet_payload_bytes = packet_payload_bytes;
    }
}

/// Bulk-load write timing statistics collected by a bulk-load request.
///
/// These counters are intended for benchmarking and diagnostics. They separate
/// time spent in bulk-load packet draining from lower-level connection writes
/// and flushes without changing bulk-load behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BulkLoadWriteTimingStats {
    /// Time spent inside bulk-load packet drain attempts.
    pub write_packets_elapsed: Duration,
    /// Number of times bulk-load packet draining wrote to the connection.
    pub write_to_wire_calls: u64,
    /// Time spent awaiting lower-level connection writes from bulk load.
    pub write_to_wire_elapsed: Duration,
    /// Payload bytes passed to lower-level connection writes from bulk load.
    pub write_to_wire_payload_bytes: u64,
    /// Slowest lower-level connection write awaited by bulk load.
    pub max_write_to_wire_elapsed: Duration,
    /// Largest payload passed to a lower-level connection write from bulk load.
    pub max_write_to_wire_payload_bytes: usize,
    /// Number of bulk-load flushes.
    pub flush_calls: u64,
    /// Time spent awaiting bulk-load flushes.
    pub flush_elapsed: Duration,
    /// Slowest explicit flush awaited by bulk load.
    pub max_flush_elapsed: Duration,
    /// Time spent finalizing the bulk-load request.
    pub finalize_elapsed: Duration,
    /// Time spent awaiting the final `EndOfMessage` packet write.
    pub finalize_write_to_wire_elapsed: Duration,
    /// Time spent awaiting the final explicit flush.
    pub finalize_flush_elapsed: Duration,
    /// Time spent waiting for the server result after final bulk packet flush.
    pub finalize_result_elapsed: Duration,
    /// Breakdown of bulk-load connection writes below the coarse
    /// `write_to_wire` aggregate.
    pub connection_write: BulkLoadConnectionWriteStats,
    /// Experimental raw-bulk direct packet write statistics.
    pub direct_packet_write: BulkLoadDirectPacketWriteStats,
}

impl BulkLoadWriteTimingStats {
    fn record_write_packets_elapsed(&mut self, elapsed: Duration) {
        self.write_packets_elapsed += elapsed;
    }

    fn record_write_to_wire(&mut self, elapsed: Duration, payload_bytes: usize) {
        self.write_to_wire_calls = self.write_to_wire_calls.saturating_add(1);
        self.write_to_wire_elapsed += elapsed;
        self.write_to_wire_payload_bytes = self
            .write_to_wire_payload_bytes
            .saturating_add(usize_to_u64_saturating(payload_bytes));
        self.max_write_to_wire_elapsed = self.max_write_to_wire_elapsed.max(elapsed);
        self.max_write_to_wire_payload_bytes =
            self.max_write_to_wire_payload_bytes.max(payload_bytes);
    }

    fn record_flush(&mut self, elapsed: Duration) {
        self.flush_calls = self.flush_calls.saturating_add(1);
        self.flush_elapsed += elapsed;
        self.max_flush_elapsed = self.max_flush_elapsed.max(elapsed);
    }

    fn record_finalize_elapsed(&mut self, elapsed: Duration) {
        self.finalize_elapsed += elapsed;
    }

    fn record_finalize_write_to_wire_elapsed(&mut self, elapsed: Duration) {
        self.finalize_write_to_wire_elapsed += elapsed;
    }

    fn record_finalize_flush_elapsed(&mut self, elapsed: Duration) {
        self.finalize_flush_elapsed += elapsed;
    }

    fn record_finalize_result_elapsed(&mut self, elapsed: Duration) {
        self.finalize_result_elapsed += elapsed;
    }

    fn record_connection_write(
        &mut self,
        payload_bytes: usize,
        ready_elapsed: Duration,
        encode_elapsed: Duration,
        flush_elapsed: Duration,
    ) {
        self.connection_write
            .record(payload_bytes, ready_elapsed, encode_elapsed, flush_elapsed);
    }

    fn record_direct_packet_write(&mut self, timing: DirectPacketWriteTiming) {
        self.direct_packet_write.record_timing(timing, false);
    }

    fn record_direct_final_packet_write(&mut self, timing: DirectPacketWriteTiming) {
        self.direct_packet_write.record_timing(timing, true);
    }
}

/// Detailed timing statistics for bulk-load writes through the framed
/// connection sink.
///
/// These counters are a diagnostic breakdown under
/// [`BulkLoadWriteTimingStats::write_to_wire_elapsed`]. They are intended to
/// show whether raw bulk writes are dominated by sink readiness, packet
/// encoding, or sink flushing while preserving the existing write behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BulkLoadConnectionWriteStats {
    /// Number of bulk-load packets passed into the connection write path.
    pub calls: u64,
    /// Payload bytes passed into the connection write path, excluding TDS
    /// packet headers.
    pub payload_bytes: u64,
    /// Time spent waiting for the framed sink to accept another packet.
    pub ready_elapsed: Duration,
    /// Time spent encoding packets into the framed sink buffer.
    pub encode_elapsed: Duration,
    /// Time spent flushing the framed sink after packet encoding.
    pub flush_elapsed: Duration,
    /// Slowest framed sink readiness wait.
    pub max_ready_elapsed: Duration,
    /// Slowest packet encode operation.
    pub max_encode_elapsed: Duration,
    /// Slowest framed sink flush.
    pub max_flush_elapsed: Duration,
    /// Largest payload passed into the connection write path.
    pub max_payload_bytes: usize,
}

impl BulkLoadConnectionWriteStats {
    fn record(
        &mut self,
        payload_bytes: usize,
        ready_elapsed: Duration,
        encode_elapsed: Duration,
        flush_elapsed: Duration,
    ) {
        self.calls = self.calls.saturating_add(1);
        self.payload_bytes = self
            .payload_bytes
            .saturating_add(usize_to_u64_saturating(payload_bytes));
        self.ready_elapsed += ready_elapsed;
        self.encode_elapsed += encode_elapsed;
        self.flush_elapsed += flush_elapsed;
        self.max_ready_elapsed = self.max_ready_elapsed.max(ready_elapsed);
        self.max_encode_elapsed = self.max_encode_elapsed.max(encode_elapsed);
        self.max_flush_elapsed = self.max_flush_elapsed.max(flush_elapsed);
        self.max_payload_bytes = self.max_payload_bytes.max(payload_bytes);
    }
}

/// Detailed timing statistics for an experimental raw-bulk direct packet
/// writer.
///
/// These counters are intended to compare an experimental bulk-only packet
/// writer against the framed sink path. They remain zero when the framed path
/// is used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BulkLoadDirectPacketWriteStats {
    /// Number of TDS packets passed to the direct packet writer.
    pub calls: u64,
    /// Payload bytes passed to the direct packet writer, excluding headers.
    pub payload_bytes: u64,
    /// Header bytes written by the direct packet writer.
    pub header_bytes: u64,
    /// Largest payload passed to the direct packet writer.
    pub max_payload_bytes: usize,
    /// Number of final `EndOfMessage` packets passed to the direct packet writer.
    pub final_calls: u64,
    /// Payload bytes in final `EndOfMessage` packets.
    pub final_payload_bytes: u64,
    /// Header bytes in final `EndOfMessage` packets.
    pub final_header_bytes: u64,
    /// Direct packet writes observed on a raw, non-TLS stream.
    pub raw_stream_calls: u64,
    /// Direct packet writes observed on a TLS stream.
    pub tls_stream_calls: u64,
    /// Number of lower-level write calls issued by the direct packet writer.
    pub write_calls: u64,
    /// Bytes accepted by lower-level writes, including headers and payloads.
    pub write_bytes: u64,
    /// Largest byte count accepted by a single lower-level write.
    pub max_write_bytes: usize,
    /// Time spent awaiting lower-level writes.
    pub write_elapsed: Duration,
    /// Slowest lower-level write.
    pub max_write_elapsed: Duration,
    /// Number of lower-level writes used for packet headers.
    pub header_write_calls: u64,
    /// Header bytes accepted by lower-level writes.
    pub header_write_bytes: u64,
    /// Largest header byte count accepted by a single lower-level write.
    pub header_max_write_bytes: usize,
    /// Time spent awaiting lower-level header writes.
    pub header_write_elapsed: Duration,
    /// Slowest lower-level header write.
    pub header_max_write_elapsed: Duration,
    /// Header writes that accepted fewer bytes than remained in the header slice.
    pub header_partial_writes: u64,
    /// Number of lower-level writes used for packet payloads.
    pub payload_write_calls: u64,
    /// Payload bytes accepted by lower-level writes.
    pub payload_write_bytes: u64,
    /// Largest payload byte count accepted by a single lower-level write.
    pub payload_max_write_bytes: usize,
    /// Time spent awaiting lower-level payload writes.
    pub payload_write_elapsed: Duration,
    /// Slowest lower-level payload write.
    pub payload_max_write_elapsed: Duration,
    /// Payload writes that accepted fewer bytes than remained in the payload slice.
    pub payload_partial_writes: u64,
    /// Number of low-level `poll_write` attempts.
    pub poll_write_polls: u64,
    /// Number of `poll_write` attempts that returned `Pending`.
    pub poll_write_pending_count: u64,
    /// Time spent waiting after `poll_write` returned `Pending`.
    pub poll_write_pending_elapsed: Duration,
    /// Slowest wait after a `poll_write` returned `Pending`.
    pub poll_write_max_pending_elapsed: Duration,
    /// Number of `poll_write` attempts that returned ready with a write result.
    pub poll_write_ready_count: u64,
    /// Time spent in ready `poll_write` attempts.
    pub poll_write_ready_elapsed: Duration,
    /// Slowest ready `poll_write` attempt.
    pub poll_write_max_ready_elapsed: Duration,
    /// Number of explicit direct packet writer flushes.
    pub flush_calls: u64,
    /// Time spent awaiting explicit direct packet writer flushes.
    pub flush_elapsed: Duration,
    /// Slowest explicit direct packet writer flush.
    pub max_flush_elapsed: Duration,
    /// Number of direct packet flush polls that returned `Pending`.
    pub flush_pending_count: u64,
    /// Time spent waiting after direct packet flush polls returned `Pending`.
    pub flush_pending_elapsed: Duration,
    /// Slowest wait after a direct packet flush poll returned `Pending`.
    pub flush_max_pending_elapsed: Duration,
}

impl BulkLoadDirectPacketWriteStats {
    fn record_timing(&mut self, timing: DirectPacketWriteTiming, final_packet: bool) {
        self.record_packet(timing.payload_bytes, timing.header_bytes, final_packet);
        self.record_stream_mode(timing.raw_stream, timing.tls_stream);
        self.record_write_summary(
            timing.write_calls,
            timing.write_bytes,
            timing.max_write_bytes,
            timing.write_elapsed,
            timing.max_write_elapsed,
        );
        self.record_header_write_summary(
            timing.header_write_calls,
            timing.header_write_bytes,
            timing.header_max_write_bytes,
            timing.header_write_elapsed,
            timing.header_max_write_elapsed,
            timing.header_partial_writes,
        );
        self.record_payload_write_summary(
            timing.payload_write_calls,
            timing.payload_write_bytes,
            timing.payload_max_write_bytes,
            timing.payload_write_elapsed,
            timing.payload_max_write_elapsed,
            timing.payload_partial_writes,
        );
        self.record_poll_write_summary(
            timing.poll_write_polls,
            timing.poll_write_pending_count,
            timing.poll_write_pending_elapsed,
            timing.poll_write_max_pending_elapsed,
            timing.poll_write_ready_count,
            timing.poll_write_ready_elapsed,
            timing.poll_write_max_ready_elapsed,
        );
        self.record_flush(
            timing.flush_elapsed,
            timing.flush_pending_count,
            timing.flush_pending_elapsed,
            timing.flush_max_pending_elapsed,
        );
    }

    fn record_packet(&mut self, payload_bytes: usize, header_bytes: usize, final_packet: bool) {
        self.calls = self.calls.saturating_add(1);
        self.payload_bytes = self
            .payload_bytes
            .saturating_add(usize_to_u64_saturating(payload_bytes));
        self.header_bytes = self
            .header_bytes
            .saturating_add(usize_to_u64_saturating(header_bytes));
        self.max_payload_bytes = self.max_payload_bytes.max(payload_bytes);

        if final_packet {
            self.final_calls = self.final_calls.saturating_add(1);
            self.final_payload_bytes = self
                .final_payload_bytes
                .saturating_add(usize_to_u64_saturating(payload_bytes));
            self.final_header_bytes = self
                .final_header_bytes
                .saturating_add(usize_to_u64_saturating(header_bytes));
        }
    }

    fn record_stream_mode(&mut self, raw_stream: bool, tls_stream: bool) {
        if raw_stream {
            self.raw_stream_calls = self.raw_stream_calls.saturating_add(1);
        }
        if tls_stream {
            self.tls_stream_calls = self.tls_stream_calls.saturating_add(1);
        }
    }

    fn record_write_summary(
        &mut self,
        calls: u64,
        bytes: u64,
        max_bytes: usize,
        elapsed: Duration,
        max_elapsed: Duration,
    ) {
        self.write_calls = self.write_calls.saturating_add(calls);
        self.write_bytes = self.write_bytes.saturating_add(bytes);
        self.max_write_bytes = self.max_write_bytes.max(max_bytes);
        self.write_elapsed += elapsed;
        self.max_write_elapsed = self.max_write_elapsed.max(max_elapsed);
    }

    fn record_header_write_summary(
        &mut self,
        calls: u64,
        bytes: u64,
        max_bytes: usize,
        elapsed: Duration,
        max_elapsed: Duration,
        partial_writes: u64,
    ) {
        self.header_write_calls = self.header_write_calls.saturating_add(calls);
        self.header_write_bytes = self.header_write_bytes.saturating_add(bytes);
        self.header_max_write_bytes = self.header_max_write_bytes.max(max_bytes);
        self.header_write_elapsed += elapsed;
        self.header_max_write_elapsed = self.header_max_write_elapsed.max(max_elapsed);
        self.header_partial_writes = self.header_partial_writes.saturating_add(partial_writes);
    }

    fn record_payload_write_summary(
        &mut self,
        calls: u64,
        bytes: u64,
        max_bytes: usize,
        elapsed: Duration,
        max_elapsed: Duration,
        partial_writes: u64,
    ) {
        self.payload_write_calls = self.payload_write_calls.saturating_add(calls);
        self.payload_write_bytes = self.payload_write_bytes.saturating_add(bytes);
        self.payload_max_write_bytes = self.payload_max_write_bytes.max(max_bytes);
        self.payload_write_elapsed += elapsed;
        self.payload_max_write_elapsed = self.payload_max_write_elapsed.max(max_elapsed);
        self.payload_partial_writes = self.payload_partial_writes.saturating_add(partial_writes);
    }

    fn record_poll_write_summary(
        &mut self,
        polls: u64,
        pending_count: u64,
        pending_elapsed: Duration,
        max_pending_elapsed: Duration,
        ready_count: u64,
        ready_elapsed: Duration,
        max_ready_elapsed: Duration,
    ) {
        self.poll_write_polls = self.poll_write_polls.saturating_add(polls);
        self.poll_write_pending_count = self.poll_write_pending_count.saturating_add(pending_count);
        self.poll_write_pending_elapsed += pending_elapsed;
        self.poll_write_max_pending_elapsed =
            self.poll_write_max_pending_elapsed.max(max_pending_elapsed);
        self.poll_write_ready_count = self.poll_write_ready_count.saturating_add(ready_count);
        self.poll_write_ready_elapsed += ready_elapsed;
        self.poll_write_max_ready_elapsed =
            self.poll_write_max_ready_elapsed.max(max_ready_elapsed);
    }

    fn record_flush(
        &mut self,
        elapsed: Duration,
        pending_count: u64,
        pending_elapsed: Duration,
        max_pending_elapsed: Duration,
    ) {
        self.flush_calls = self.flush_calls.saturating_add(1);
        self.flush_elapsed += elapsed;
        self.max_flush_elapsed = self.max_flush_elapsed.max(elapsed);
        self.flush_pending_count = self.flush_pending_count.saturating_add(pending_count);
        self.flush_pending_elapsed += pending_elapsed;
        self.flush_max_pending_elapsed = self.flush_max_pending_elapsed.max(max_pending_elapsed);
    }
}

/// Complete benchmark statistics collected by a bulk-load request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BulkLoadStats {
    /// Packet counters collected while writing bulk-load data.
    pub packet: BulkLoadPacketStats,
    /// Timing counters collected while writing bulk-load data.
    pub write_timing: BulkLoadWriteTimingStats,
}

/// A handler for a bulk insert data flow.
#[derive(Debug)]
pub struct BulkLoadRequest<'a, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    connection: &'a mut Connection<S>,
    packet_id: u8,
    buf: BytesMut,
    columns: Vec<MetaDataColumn<'a>>,
    packet_stats: BulkLoadPacketStats,
    write_timing_stats: BulkLoadWriteTimingStats,
    direct_packet_writes: bool,
}

impl<'a, S> BulkLoadRequest<'a, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    pub(crate) fn new(
        connection: &'a mut Connection<S>,
        columns: Vec<MetaDataColumn<'a>>,
    ) -> crate::Result<Self> {
        let packet_id = connection.context_mut().next_packet_id();
        let mut buf = BytesMut::new();

        let cmd = TokenColMetaData {
            columns: columns.clone(),
        };

        cmd.encode(&mut buf)?;

        let this = Self {
            connection,
            packet_id,
            buf,
            columns,
            packet_stats: BulkLoadPacketStats::default(),
            write_timing_stats: BulkLoadWriteTimingStats::default(),
            direct_packet_writes: false,
        };

        Ok(this)
    }

    /// Returns the destination columns used by this bulk-load request.
    ///
    /// The returned columns are ordered exactly as [`send`] expects row values
    /// to be encoded. The metadata is discovered during
    /// [`Client::bulk_insert`] and filtered to updateable destination columns
    /// before the request is created.
    ///
    /// [`send`]: Self::send
    /// [`Client::bulk_insert`]: crate::Client::bulk_insert
    pub fn columns(&self) -> impl ExactSizeIterator<Item = BulkLoadColumn<'_>> {
        bulk_load_columns(&self.columns)
    }

    /// Returns packet-write statistics collected by this bulk-load request.
    pub fn packet_stats(&self) -> BulkLoadPacketStats {
        self.packet_stats
    }

    /// Returns write timing statistics collected by this bulk-load request.
    pub fn write_timing_stats(&self) -> BulkLoadWriteTimingStats {
        self.write_timing_stats
    }

    /// Returns all benchmark statistics collected by this bulk-load request.
    pub fn stats(&self) -> BulkLoadStats {
        BulkLoadStats {
            packet: self.packet_stats,
            write_timing: self.write_timing_stats,
        }
    }

    /// Enables the experimental direct packet write path for this bulk-load
    /// request.
    ///
    /// The default path uses Tiberius' framed packet sink. This opt-in path is
    /// intended for benchmark comparisons only: it preserves TDS packet framing
    /// while writing raw bulk packets directly to the underlying transport.
    /// Normal query writes and requests that do not call this method continue
    /// using the framed sink path.
    pub fn enable_direct_packet_writes(&mut self) {
        self.direct_packet_writes = true;
    }

    /// Returns true if this request uses the experimental direct packet writer.
    pub fn direct_packet_writes_enabled(&self) -> bool {
        self.direct_packet_writes
    }

    /// Adds a new row to the bulk insert, flushing only when having a full packet of data.
    ///
    /// # Warning
    ///
    /// After the last row, [`finalize`] must be called to flush the buffered
    /// data and for the data to actually be available in the table.
    ///
    /// [`finalize`]: #method.finalize
    pub async fn send(&mut self, row: TokenRow<'a>) -> crate::Result<()> {
        let mut buf_with_columns = BytesMutWithDataColumns::new(&mut self.buf, &self.columns);

        row.encode(&mut buf_with_columns)?;
        self.write_packets().await?;

        Ok(())
    }

    /// Adds one already-encoded row value payload to the bulk insert.
    ///
    /// The payload must contain only the encoded value bytes for one row. It
    /// must not include the TDS `ROW` token byte. This method prefixes the
    /// normal `ROW` token (`0xD1`) and then appends the payload to the same
    /// packet buffer used by [`send`].
    ///
    /// Empty payloads are rejected. After the last row, [`finalize`] must be
    /// called to flush the buffered data and complete the bulk load.
    ///
    /// [`send`]: Self::send
    /// [`finalize`]: Self::finalize
    pub async fn send_raw_row_payload(&mut self, payload: impl AsRef<[u8]>) -> crate::Result<()> {
        append_raw_row_payload(&mut self.buf, payload.as_ref())?;
        self.write_packets().await?;

        Ok(())
    }

    /// Adds already-encoded complete TDS rows to the bulk insert.
    ///
    /// The payload must contain one or more complete TDS rows. Each row must
    /// begin with the TDS `ROW` token byte (`0xD1`) followed by that row's
    /// encoded value payload. This is the batched raw path intended for callers
    /// that encode many rows, such as one Arrow `RecordBatch`, before handing
    /// bytes to Tiberius.
    ///
    /// Empty payloads are rejected. This method performs only a cheap first-byte
    /// check; callers are responsible for producing semantically valid row
    /// bytes for this request's [`columns`]. After the last batch, [`finalize`]
    /// must be called to flush the buffered data and complete the bulk load.
    ///
    /// [`columns`]: Self::columns
    /// [`finalize`]: Self::finalize
    pub async fn send_raw_rows_payload(&mut self, payload: impl AsRef<[u8]>) -> crate::Result<()> {
        append_raw_rows_payload(&mut self.buf, payload.as_ref())?;
        self.write_packets().await?;

        Ok(())
    }

    /// Adds already-encoded complete TDS rows with row-token offset checks.
    ///
    /// This method has the same byte boundary as [`send_raw_rows_payload`]:
    /// `payload` must contain one or more complete TDS rows and each row must
    /// start with the TDS `ROW` token byte (`0xD1`). The `row_token_offsets`
    /// slice identifies the byte offset of every row token in `payload`.
    ///
    /// The offset checks are intended as a cheap validation layer for batched
    /// encoders. They verify that offsets are non-empty, start at zero, are
    /// strictly increasing, are in bounds, and point at `ROW` tokens. They do
    /// not parse or validate the row value payloads.
    ///
    /// [`send_raw_rows_payload`]: Self::send_raw_rows_payload
    pub async fn send_raw_rows_payload_checked(
        &mut self,
        payload: impl AsRef<[u8]>,
        row_token_offsets: impl AsRef<[usize]>,
    ) -> crate::Result<()> {
        append_raw_rows_payload_checked(
            &mut self.buf,
            payload.as_ref(),
            row_token_offsets.as_ref(),
        )?;
        self.write_packets().await?;

        Ok(())
    }

    /// Adds raw rows by encoding directly into this request's packet buffer.
    ///
    /// The closure receives the same internal buffer used by [`send`] and the
    /// other raw bulk methods. It must append one or more complete TDS rows,
    /// where each row starts with the TDS `ROW` token byte (`0xD1`), and then
    /// return [`RawRowsAppend`] with row-token offsets relative to the appended
    /// region.
    ///
    /// If the closure returns an error, or if the appended region fails
    /// row-token validation, this method truncates the request buffer back to
    /// its original length before returning the error. On success, it uses the
    /// normal bulk-load packet splitting path.
    ///
    /// [`send`]: Self::send
    pub async fn send_raw_rows_with<F>(&mut self, encode: F) -> crate::Result<()>
    where
        F: FnOnce(&mut RawRowsAppendBuffer<'_>) -> crate::Result<RawRowsAppend>,
    {
        append_raw_rows_with(&mut self.buf, encode)?;
        self.write_packets().await?;

        Ok(())
    }

    /// Ends the bulk load, flushing all pending data to the wire.
    ///
    /// This method must be called after sending all the data to flush all
    /// pending data and to get the server actually to store the rows to the
    /// table.
    pub async fn finalize(self) -> crate::Result<ExecuteResult> {
        let (result, _) = self.finalize_with_stats().await?;
        Ok(result)
    }

    /// Ends the bulk load and returns packet statistics collected by the request.
    ///
    /// This method has the same write behavior as [`finalize`], but also
    /// returns the final packet counters that are otherwise unavailable because
    /// finalization consumes the request.
    ///
    /// [`finalize`]: Self::finalize
    pub async fn finalize_with_packet_stats(
        self,
    ) -> crate::Result<(ExecuteResult, BulkLoadPacketStats)> {
        let (result, stats) = self.finalize_with_stats().await?;
        Ok((result, stats.packet))
    }

    /// Ends the bulk load and returns all benchmark statistics collected by the request.
    ///
    /// This method has the same write behavior as [`finalize`], but returns
    /// packet counters and write timing counters after finalization consumes
    /// the request.
    ///
    /// [`finalize`]: Self::finalize
    pub async fn finalize_with_stats(mut self) -> crate::Result<(ExecuteResult, BulkLoadStats)> {
        let finalize_start = Instant::now();
        TokenDone::default().encode(&mut self.buf)?;
        self.write_packets().await?;

        let mut header = PacketHeader::bulk_load(self.packet_id);
        header.set_status(PacketStatus::EndOfMessage);

        let data = self.buf.split();
        self.packet_stats.record_finalized_packet(data.len());

        event!(
            Level::TRACE,
            "Finalizing a bulk insert ({} bytes)",
            data.len() + HEADER_BYTES,
        );

        let data_len = data.len();
        let write_start = Instant::now();
        let write_result = if self.direct_packet_writes {
            self.connection
                .write_direct_packet_with_timing(header, &data)
                .await
                .map(EitherWriteTiming::Direct)
        } else {
            self.connection
                .write_to_wire_with_timing(header, data)
                .await
                .map(EitherWriteTiming::Framed)
        };
        let write_elapsed = write_start.elapsed();
        self.write_timing_stats
            .record_write_to_wire(write_elapsed, data_len);
        match write_result {
            Ok(EitherWriteTiming::Framed(connection_timing)) => {
                self.write_timing_stats.record_connection_write(
                    data_len,
                    connection_timing.ready_elapsed,
                    connection_timing.encode_elapsed,
                    connection_timing.flush_elapsed,
                );
                self.write_timing_stats
                    .record_finalize_write_to_wire_elapsed(write_elapsed);
            }
            Ok(EitherWriteTiming::Direct(direct_timing)) => {
                self.write_timing_stats
                    .record_direct_final_packet_write(direct_timing);
                self.write_timing_stats
                    .record_finalize_write_to_wire_elapsed(write_elapsed);
            }
            Err(err) => return Err(err),
        }

        let flush_start = Instant::now();
        let flush_result = self.connection.flush_sink().await;
        let flush_elapsed = flush_start.elapsed();
        self.write_timing_stats.record_flush(flush_elapsed);
        self.write_timing_stats
            .record_finalize_flush_elapsed(flush_elapsed);
        flush_result?;

        let result_start = Instant::now();
        let result = ExecuteResult::new(self.connection).await?;
        self.write_timing_stats
            .record_finalize_result_elapsed(result_start.elapsed());
        self.write_timing_stats
            .record_finalize_elapsed(finalize_start.elapsed());
        let stats = self.stats();

        Ok((result, stats))
    }

    async fn write_packets(&mut self) -> crate::Result<()> {
        let write_packets_start = Instant::now();
        let result = if self.direct_packet_writes {
            self.write_packets_direct_inner().await
        } else {
            self.write_packets_framed_inner().await
        };
        self.write_timing_stats
            .record_write_packets_elapsed(write_packets_start.elapsed());
        result
    }

    async fn write_packets_framed_inner(&mut self) -> crate::Result<()> {
        let packet_size = (self.connection.context().packet_size() as usize) - HEADER_BYTES;
        self.packet_stats.record_write_packets_call(self.buf.len());

        while self.buf.len() > packet_size {
            let header = PacketHeader::bulk_load(self.packet_id);
            let data = self.buf.split_to(packet_size);
            self.packet_stats.record_packet_written(data.len());

            event!(
                Level::TRACE,
                "Bulk insert packet ({} bytes)",
                data.len() + HEADER_BYTES,
            );

            let data_len = data.len();
            let write_start = Instant::now();
            let write_result = self
                .connection
                .write_to_wire_with_timing(header, data)
                .await;
            self.write_timing_stats
                .record_write_to_wire(write_start.elapsed(), data_len);
            match write_result {
                Ok(connection_timing) => {
                    self.write_timing_stats.record_connection_write(
                        data_len,
                        connection_timing.ready_elapsed,
                        connection_timing.encode_elapsed,
                        connection_timing.flush_elapsed,
                    );
                }
                Err(err) => return Err(err),
            }
        }

        self.packet_stats
            .record_buffered_bytes_after_write(self.buf.len());

        Ok(())
    }

    async fn write_packets_direct_inner(&mut self) -> crate::Result<()> {
        let packet_size = (self.connection.context().packet_size() as usize) - HEADER_BYTES;
        self.packet_stats.record_write_packets_call(self.buf.len());

        while self.buf.len() > packet_size {
            let header = PacketHeader::bulk_load(self.packet_id);
            let data = self.buf.split_to(packet_size);
            self.packet_stats.record_packet_written(data.len());

            event!(
                Level::TRACE,
                "Bulk insert direct packet ({} bytes)",
                data.len() + HEADER_BYTES,
            );

            let data_len = data.len();
            let write_start = Instant::now();
            let write_result = self
                .connection
                .write_direct_packet_with_timing(header, &data)
                .await;
            self.write_timing_stats
                .record_write_to_wire(write_start.elapsed(), data_len);
            match write_result {
                Ok(direct_timing) => self
                    .write_timing_stats
                    .record_direct_packet_write(direct_timing),
                Err(err) => return Err(err),
            }
        }

        self.packet_stats
            .record_buffered_bytes_after_write(self.buf.len());

        Ok(())
    }
}

enum EitherWriteTiming {
    Framed(crate::client::ConnectionWriteTiming),
    Direct(DirectPacketWriteTiming),
}

fn bulk_load_columns<'a>(
    columns: &'a [MetaDataColumn<'a>],
) -> impl ExactSizeIterator<Item = BulkLoadColumn<'a>> {
    columns
        .iter()
        .enumerate()
        .map(|(ordinal, column)| BulkLoadColumn { ordinal, column })
}

fn append_raw_row_payload(buf: &mut BytesMut, payload: &[u8]) -> crate::Result<()> {
    if payload.is_empty() {
        return Err(crate::Error::BulkInput(
            "raw bulk row payload cannot be empty".into(),
        ));
    }

    buf.put_u8(TokenType::Row as u8);
    buf.extend_from_slice(payload);

    Ok(())
}

fn append_raw_rows_payload(buf: &mut BytesMut, payload: &[u8]) -> crate::Result<()> {
    if payload.is_empty() {
        return Err(crate::Error::BulkInput(
            "raw bulk rows payload cannot be empty".into(),
        ));
    }

    if payload[0] != TokenType::Row as u8 {
        return Err(crate::Error::BulkInput(
            "raw bulk rows payload must start with a TDS ROW token".into(),
        ));
    }

    buf.extend_from_slice(payload);

    Ok(())
}

fn append_raw_rows_payload_checked(
    buf: &mut BytesMut,
    payload: &[u8],
    row_token_offsets: &[usize],
) -> crate::Result<()> {
    validate_raw_row_token_offsets(payload, row_token_offsets)?;
    buf.extend_from_slice(payload);

    Ok(())
}

/// Runs a rollback-safe append transaction against the raw bulk request buffer.
///
/// `buf` is the existing `BulkLoadRequest` buffer. It may already contain
/// column metadata, previously buffered rows, or both. The `encode` closure is
/// responsible for appending new complete TDS rows through
/// `RawRowsAppendBuffer`, then returning row-token offsets relative to only the
/// bytes it appended.
///
/// If encoding or validation fails, this helper truncates `buf` back to the
/// original length so the request can continue to behave as if the attempted
/// append never happened.
fn append_raw_rows_with<F>(buf: &mut BytesMut, encode: F) -> crate::Result<()>
where
    F: FnOnce(&mut RawRowsAppendBuffer<'_>) -> crate::Result<RawRowsAppend>,
{
    // Everything after this byte offset belongs to the attempted append.
    let start_len = buf.len();
    let mut raw_buf = RawRowsAppendBuffer { bytes: buf };

    // The caller writes row bytes into `raw_buf` here.
    let append = match encode(&mut raw_buf) {
        Ok(append) => append,
        Err(err) => {
            raw_buf.bytes.truncate(start_len);
            return Err(err);
        }
    };

    // Validate only the bytes appended by this call. Returned offsets are
    // relative to `raw_buf.bytes[start_len..]`, not the full request buffer.
    if let Err(err) =
        validate_raw_row_token_offsets(&raw_buf.bytes[start_len..], append.row_token_offsets())
    {
        raw_buf.bytes.truncate(start_len);
        return Err(err);
    }

    Ok(())
}

fn usize_to_u64_saturating(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn validate_raw_row_token_offsets(
    payload: &[u8],
    row_token_offsets: &[usize],
) -> crate::Result<()> {
    if payload.is_empty() {
        return Err(crate::Error::BulkInput(
            "raw bulk rows payload cannot be empty".into(),
        ));
    }

    if row_token_offsets.is_empty() {
        return Err(crate::Error::BulkInput(
            "raw bulk row token offsets cannot be empty".into(),
        ));
    }

    if row_token_offsets[0] != 0 {
        return Err(crate::Error::BulkInput(
            "raw bulk row token offsets must start at zero".into(),
        ));
    }

    let mut previous = None;

    for &offset in row_token_offsets {
        if offset >= payload.len() {
            return Err(crate::Error::BulkInput(
                "raw bulk row token offset is out of bounds".into(),
            ));
        }

        if previous.is_some_and(|previous| offset <= previous) {
            return Err(crate::Error::BulkInput(
                "raw bulk row token offsets must be strictly increasing".into(),
            ));
        }

        if payload[offset] != TokenType::Row as u8 {
            return Err(crate::Error::BulkInput(
                "raw bulk row token offset must point to a TDS ROW token".into(),
            ));
        }

        previous = Some(offset);
    }

    Ok(())
}

/// Read-only destination metadata for one bulk-load column.
#[derive(Debug, Clone, Copy)]
pub struct BulkLoadColumn<'a> {
    ordinal: usize,
    column: &'a MetaDataColumn<'a>,
}

impl BulkLoadColumn<'_> {
    /// The zero-based ordinal in the bulk row encoding order.
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// The destination column name.
    pub fn name(&self) -> &str {
        &self.column.col_name
    }

    /// The coarse logical column type.
    pub fn column_type(&self) -> ColumnType {
        ColumnType::from(&self.column.base.ty)
    }

    /// The raw TDS column flags reported for this destination column.
    pub fn flags(&self) -> BitFlags<ColumnFlag> {
        self.column.base.flags
    }

    /// Whether this destination column accepts null values.
    pub fn is_nullable(&self) -> bool {
        self.flags().contains(ColumnFlag::Nullable)
    }

    /// Whether this destination column is updateable by bulk load.
    pub fn is_updateable(&self) -> bool {
        self.flags().contains(ColumnFlag::Updateable)
    }

    /// The detailed TDS type metadata for direct bulk-row encoding.
    pub fn type_info(&self) -> &TypeInfo {
        &self.column.base.ty
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::tds::codec::{BaseMetaDataColumn, FixedLenType};

    #[test]
    fn exposes_bulk_load_column_metadata() {
        let metadata = MetaDataColumn {
            base: BaseMetaDataColumn {
                flags: ColumnFlag::Nullable | ColumnFlag::Updateable,
                ty: TypeInfo::FixedLen(FixedLenType::Int4),
            },
            col_name: Cow::Borrowed("value"),
        };

        let column = BulkLoadColumn {
            ordinal: 2,
            column: &metadata,
        };

        assert_eq!(2, column.ordinal());
        assert_eq!("value", column.name());
        assert_eq!(ColumnType::Int4, column.column_type());
        assert!(column.is_nullable());
        assert!(column.is_updateable());
        assert!(column.flags().contains(ColumnFlag::Nullable));
        assert_eq!(&TypeInfo::FixedLen(FixedLenType::Int4), column.type_info());
    }

    #[test]
    fn bulk_load_packet_stats_default_to_zero() {
        let stats = BulkLoadPacketStats::default();

        assert_eq!(stats.write_packets_calls, 0);
        assert_eq!(stats.packets_written, 0);
        assert_eq!(stats.packet_payload_bytes, 0);
        assert_eq!(stats.max_packet_payload_bytes, 0);
        assert_eq!(stats.max_buffered_bytes_before_write, 0);
        assert_eq!(stats.buffered_bytes_after_last_write, 0);
        assert_eq!(stats.finalized_packet_payload_bytes, 0);
    }

    #[test]
    fn bulk_load_packet_stats_record_write_attempts_and_buffer_tail() {
        let mut stats = BulkLoadPacketStats::default();

        stats.record_write_packets_call(128);
        stats.record_buffered_bytes_after_write(17);
        stats.record_write_packets_call(64);
        stats.record_buffered_bytes_after_write(3);

        assert_eq!(stats.write_packets_calls, 2);
        assert_eq!(stats.max_buffered_bytes_before_write, 128);
        assert_eq!(stats.buffered_bytes_after_last_write, 3);
        assert_eq!(stats.packets_written, 0);
    }

    #[test]
    fn bulk_load_packet_stats_accumulate_packet_bytes_and_maxima() {
        let mut stats = BulkLoadPacketStats::default();

        stats.record_packet_written(4);
        stats.record_packet_written(9);
        stats.record_packet_written(7);

        assert_eq!(stats.packets_written, 3);
        assert_eq!(stats.packet_payload_bytes, 20);
        assert_eq!(stats.max_packet_payload_bytes, 9);
    }

    #[test]
    fn bulk_load_packet_stats_record_final_packet_separately() {
        let mut stats = BulkLoadPacketStats::default();

        stats.record_packet_written(4096);
        stats.record_finalized_packet(41);

        assert_eq!(stats.packet_payload_bytes, 4096);
        assert_eq!(stats.finalized_packet_payload_bytes, 41);
    }

    #[test]
    fn bulk_load_write_timing_stats_default_to_zero() {
        let stats = BulkLoadWriteTimingStats::default();

        assert_eq!(stats.write_packets_elapsed, Duration::ZERO);
        assert_eq!(stats.write_to_wire_calls, 0);
        assert_eq!(stats.write_to_wire_elapsed, Duration::ZERO);
        assert_eq!(stats.write_to_wire_payload_bytes, 0);
        assert_eq!(stats.max_write_to_wire_elapsed, Duration::ZERO);
        assert_eq!(stats.max_write_to_wire_payload_bytes, 0);
        assert_eq!(stats.flush_calls, 0);
        assert_eq!(stats.flush_elapsed, Duration::ZERO);
        assert_eq!(stats.max_flush_elapsed, Duration::ZERO);
        assert_eq!(stats.finalize_elapsed, Duration::ZERO);
        assert_eq!(stats.finalize_write_to_wire_elapsed, Duration::ZERO);
        assert_eq!(stats.finalize_flush_elapsed, Duration::ZERO);
        assert_eq!(stats.finalize_result_elapsed, Duration::ZERO);
        assert_eq!(
            stats.connection_write,
            BulkLoadConnectionWriteStats::default()
        );
        assert_eq!(
            stats.direct_packet_write,
            BulkLoadDirectPacketWriteStats::default()
        );
    }

    #[test]
    fn bulk_load_connection_write_stats_default_to_zero() {
        let stats = BulkLoadConnectionWriteStats::default();

        assert_eq!(stats.calls, 0);
        assert_eq!(stats.payload_bytes, 0);
        assert_eq!(stats.ready_elapsed, Duration::ZERO);
        assert_eq!(stats.encode_elapsed, Duration::ZERO);
        assert_eq!(stats.flush_elapsed, Duration::ZERO);
        assert_eq!(stats.max_ready_elapsed, Duration::ZERO);
        assert_eq!(stats.max_encode_elapsed, Duration::ZERO);
        assert_eq!(stats.max_flush_elapsed, Duration::ZERO);
        assert_eq!(stats.max_payload_bytes, 0);
    }

    #[test]
    fn bulk_load_connection_write_stats_accumulate_and_track_maxima() {
        let mut stats = BulkLoadConnectionWriteStats::default();

        stats.record(
            128,
            Duration::from_millis(3),
            Duration::from_millis(5),
            Duration::from_millis(7),
        );
        stats.record(
            256,
            Duration::from_millis(11),
            Duration::from_millis(2),
            Duration::from_millis(13),
        );

        assert_eq!(stats.calls, 2);
        assert_eq!(stats.payload_bytes, 384);
        assert_eq!(stats.ready_elapsed, Duration::from_millis(14));
        assert_eq!(stats.encode_elapsed, Duration::from_millis(7));
        assert_eq!(stats.flush_elapsed, Duration::from_millis(20));
        assert_eq!(stats.max_ready_elapsed, Duration::from_millis(11));
        assert_eq!(stats.max_encode_elapsed, Duration::from_millis(5));
        assert_eq!(stats.max_flush_elapsed, Duration::from_millis(13));
        assert_eq!(stats.max_payload_bytes, 256);
    }

    #[test]
    fn bulk_load_direct_packet_write_stats_default_to_zero() {
        let stats = BulkLoadDirectPacketWriteStats::default();

        assert_eq!(stats.calls, 0);
        assert_eq!(stats.payload_bytes, 0);
        assert_eq!(stats.header_bytes, 0);
        assert_eq!(stats.max_payload_bytes, 0);
        assert_eq!(stats.final_calls, 0);
        assert_eq!(stats.final_payload_bytes, 0);
        assert_eq!(stats.final_header_bytes, 0);
        assert_eq!(stats.raw_stream_calls, 0);
        assert_eq!(stats.tls_stream_calls, 0);
        assert_eq!(stats.write_calls, 0);
        assert_eq!(stats.write_bytes, 0);
        assert_eq!(stats.max_write_bytes, 0);
        assert_eq!(stats.write_elapsed, Duration::ZERO);
        assert_eq!(stats.max_write_elapsed, Duration::ZERO);
        assert_eq!(stats.header_write_calls, 0);
        assert_eq!(stats.header_write_bytes, 0);
        assert_eq!(stats.header_max_write_bytes, 0);
        assert_eq!(stats.header_write_elapsed, Duration::ZERO);
        assert_eq!(stats.header_max_write_elapsed, Duration::ZERO);
        assert_eq!(stats.header_partial_writes, 0);
        assert_eq!(stats.payload_write_calls, 0);
        assert_eq!(stats.payload_write_bytes, 0);
        assert_eq!(stats.payload_max_write_bytes, 0);
        assert_eq!(stats.payload_write_elapsed, Duration::ZERO);
        assert_eq!(stats.payload_max_write_elapsed, Duration::ZERO);
        assert_eq!(stats.payload_partial_writes, 0);
        assert_eq!(stats.poll_write_polls, 0);
        assert_eq!(stats.poll_write_pending_count, 0);
        assert_eq!(stats.poll_write_pending_elapsed, Duration::ZERO);
        assert_eq!(stats.poll_write_max_pending_elapsed, Duration::ZERO);
        assert_eq!(stats.poll_write_ready_count, 0);
        assert_eq!(stats.poll_write_ready_elapsed, Duration::ZERO);
        assert_eq!(stats.poll_write_max_ready_elapsed, Duration::ZERO);
        assert_eq!(stats.flush_calls, 0);
        assert_eq!(stats.flush_elapsed, Duration::ZERO);
        assert_eq!(stats.max_flush_elapsed, Duration::ZERO);
        assert_eq!(stats.flush_pending_count, 0);
        assert_eq!(stats.flush_pending_elapsed, Duration::ZERO);
        assert_eq!(stats.flush_max_pending_elapsed, Duration::ZERO);
    }

    #[test]
    fn bulk_load_direct_packet_write_stats_accumulate_and_track_maxima() {
        let mut stats = BulkLoadDirectPacketWriteStats::default();

        stats.record_packet(128, HEADER_BYTES, false);
        stats.record_packet(256, HEADER_BYTES, true);
        stats.record_stream_mode(true, false);
        stats.record_stream_mode(false, true);
        stats.record_write_summary(
            2,
            576,
            512,
            Duration::from_millis(14),
            Duration::from_millis(11),
        );
        stats.record_header_write_summary(
            3,
            24,
            HEADER_BYTES,
            Duration::from_millis(17),
            Duration::from_millis(13),
            1,
        );
        stats.record_payload_write_summary(
            5,
            552,
            384,
            Duration::from_millis(19),
            Duration::from_millis(15),
            2,
        );
        stats.record_poll_write_summary(
            11,
            7,
            Duration::from_millis(23),
            Duration::from_millis(17),
            4,
            Duration::from_millis(29),
            Duration::from_millis(19),
        );
        stats.record_flush(
            Duration::from_millis(5),
            2,
            Duration::from_millis(3),
            Duration::from_millis(2),
        );
        stats.record_flush(
            Duration::from_millis(7),
            3,
            Duration::from_millis(4),
            Duration::from_millis(3),
        );

        assert_eq!(stats.calls, 2);
        assert_eq!(stats.payload_bytes, 384);
        assert_eq!(stats.header_bytes, u64::try_from(HEADER_BYTES * 2).unwrap());
        assert_eq!(stats.max_payload_bytes, 256);
        assert_eq!(stats.final_calls, 1);
        assert_eq!(stats.final_payload_bytes, 256);
        assert_eq!(
            stats.final_header_bytes,
            u64::try_from(HEADER_BYTES).unwrap()
        );
        assert_eq!(stats.raw_stream_calls, 1);
        assert_eq!(stats.tls_stream_calls, 1);
        assert_eq!(stats.write_calls, 2);
        assert_eq!(stats.write_bytes, 576);
        assert_eq!(stats.max_write_bytes, 512);
        assert_eq!(stats.write_elapsed, Duration::from_millis(14));
        assert_eq!(stats.max_write_elapsed, Duration::from_millis(11));
        assert_eq!(stats.header_write_calls, 3);
        assert_eq!(stats.header_write_bytes, 24);
        assert_eq!(stats.header_max_write_bytes, HEADER_BYTES);
        assert_eq!(stats.header_write_elapsed, Duration::from_millis(17));
        assert_eq!(stats.header_max_write_elapsed, Duration::from_millis(13));
        assert_eq!(stats.header_partial_writes, 1);
        assert_eq!(stats.payload_write_calls, 5);
        assert_eq!(stats.payload_write_bytes, 552);
        assert_eq!(stats.payload_max_write_bytes, 384);
        assert_eq!(stats.payload_write_elapsed, Duration::from_millis(19));
        assert_eq!(stats.payload_max_write_elapsed, Duration::from_millis(15));
        assert_eq!(stats.payload_partial_writes, 2);
        assert_eq!(stats.poll_write_polls, 11);
        assert_eq!(stats.poll_write_pending_count, 7);
        assert_eq!(stats.poll_write_pending_elapsed, Duration::from_millis(23));
        assert_eq!(
            stats.poll_write_max_pending_elapsed,
            Duration::from_millis(17)
        );
        assert_eq!(stats.poll_write_ready_count, 4);
        assert_eq!(stats.poll_write_ready_elapsed, Duration::from_millis(29));
        assert_eq!(
            stats.poll_write_max_ready_elapsed,
            Duration::from_millis(19)
        );
        assert_eq!(stats.flush_calls, 2);
        assert_eq!(stats.flush_elapsed, Duration::from_millis(12));
        assert_eq!(stats.max_flush_elapsed, Duration::from_millis(7));
        assert_eq!(stats.flush_pending_count, 5);
        assert_eq!(stats.flush_pending_elapsed, Duration::from_millis(7));
        assert_eq!(stats.flush_max_pending_elapsed, Duration::from_millis(3));
    }

    #[test]
    fn bulk_load_write_timing_stats_accumulate_write_packets_elapsed() {
        let mut stats = BulkLoadWriteTimingStats::default();

        stats.record_write_packets_elapsed(Duration::from_millis(7));
        stats.record_write_packets_elapsed(Duration::from_millis(11));

        assert_eq!(stats.write_packets_elapsed, Duration::from_millis(18));
    }

    #[test]
    fn bulk_load_write_timing_stats_accumulate_write_to_wire() {
        let mut stats = BulkLoadWriteTimingStats::default();

        stats.record_write_to_wire(Duration::from_millis(13), 128);
        stats.record_write_to_wire(Duration::from_millis(17), 256);

        assert_eq!(stats.write_to_wire_calls, 2);
        assert_eq!(stats.write_to_wire_elapsed, Duration::from_millis(30));
        assert_eq!(stats.write_to_wire_payload_bytes, 384);
        assert_eq!(stats.max_write_to_wire_elapsed, Duration::from_millis(17));
        assert_eq!(stats.max_write_to_wire_payload_bytes, 256);
    }

    #[test]
    fn bulk_load_write_timing_stats_accumulate_flush_and_finalize() {
        let mut stats = BulkLoadWriteTimingStats::default();

        stats.record_flush(Duration::from_millis(19));
        stats.record_flush(Duration::from_millis(23));
        stats.record_finalize_elapsed(Duration::from_millis(29));
        stats.record_finalize_elapsed(Duration::from_millis(31));
        stats.record_finalize_write_to_wire_elapsed(Duration::from_millis(37));
        stats.record_finalize_flush_elapsed(Duration::from_millis(41));
        stats.record_finalize_result_elapsed(Duration::from_millis(43));
        stats.record_connection_write(
            128,
            Duration::from_millis(47),
            Duration::from_millis(53),
            Duration::from_millis(59),
        );

        assert_eq!(stats.flush_calls, 2);
        assert_eq!(stats.flush_elapsed, Duration::from_millis(42));
        assert_eq!(stats.max_flush_elapsed, Duration::from_millis(23));
        assert_eq!(stats.finalize_elapsed, Duration::from_millis(60));
        assert_eq!(
            stats.finalize_write_to_wire_elapsed,
            Duration::from_millis(37)
        );
        assert_eq!(stats.finalize_flush_elapsed, Duration::from_millis(41));
        assert_eq!(stats.finalize_result_elapsed, Duration::from_millis(43));
        assert_eq!(stats.connection_write.calls, 1);
        assert_eq!(stats.connection_write.payload_bytes, 128);
        assert_eq!(
            stats.connection_write.ready_elapsed,
            Duration::from_millis(47)
        );
        assert_eq!(
            stats.connection_write.encode_elapsed,
            Duration::from_millis(53)
        );
        assert_eq!(
            stats.connection_write.flush_elapsed,
            Duration::from_millis(59)
        );
    }

    #[test]
    fn appends_single_raw_row_payload_with_row_token() {
        let mut buf = BytesMut::new();

        append_raw_row_payload(&mut buf, &[0x01, 0x02, 0x03]).expect("payload should append");

        assert_eq!(&[TokenType::Row as u8, 0x01, 0x02, 0x03], &buf[..]);
    }

    #[test]
    fn rejects_empty_single_raw_row_payload() {
        let mut buf = BytesMut::new();

        append_raw_row_payload(&mut buf, &[]).expect_err("empty payload should fail");

        assert!(buf.is_empty());
    }

    #[test]
    fn appends_batched_raw_rows_payload_unchanged() {
        let mut buf = BytesMut::new();
        let payload = [TokenType::Row as u8, 0x01, TokenType::Row as u8, 0x02, 0x03];

        append_raw_rows_payload(&mut buf, &payload).expect("payload should append");

        assert_eq!(&payload, &buf[..]);
    }

    #[test]
    fn rejects_empty_batched_raw_rows_payload() {
        let mut buf = BytesMut::new();

        append_raw_rows_payload(&mut buf, &[]).expect_err("empty payload should fail");

        assert!(buf.is_empty());
    }

    #[test]
    fn rejects_batched_raw_rows_payload_without_row_token() {
        let mut buf = BytesMut::new();

        append_raw_rows_payload(&mut buf, &[0x01, 0x02])
            .expect_err("payload without row token should fail");

        assert!(buf.is_empty());
    }

    #[test]
    fn appends_checked_batched_raw_rows_payload_unchanged() {
        let mut buf = BytesMut::new();
        let payload = [TokenType::Row as u8, 0x01, TokenType::Row as u8, 0x02, 0x03];

        append_raw_rows_payload_checked(&mut buf, &payload, &[0, 2])
            .expect("payload should append");

        assert_eq!(&payload, &buf[..]);
    }

    #[test]
    fn appends_raw_rows_with_relative_offsets_after_existing_bytes() {
        let mut buf = BytesMut::from(&b"prefix"[..]);
        let payload = [TokenType::Row as u8, 0x01, TokenType::Row as u8, 0x02, 0x03];

        append_raw_rows_with(&mut buf, |buf| {
            buf.extend_from_slice(&payload);
            Ok(RawRowsAppend::new(vec![0, 2]))
        })
        .expect("payload should append");

        assert_eq!(&b"prefix"[..], &buf[..6]);
        assert_eq!(&payload, &buf[6..]);
    }

    #[test]
    fn rolls_back_raw_rows_with_closure_error() {
        let mut buf = BytesMut::from(&b"prefix"[..]);

        append_raw_rows_with(&mut buf, |buf| {
            buf.extend_from_slice(&[TokenType::Row as u8, 0x01]);
            Err(crate::Error::BulkInput(Cow::Borrowed(
                "fake append failure",
            )))
        })
        .expect_err("closure error should fail");

        assert_eq!(&b"prefix"[..], &buf[..]);
    }

    #[test]
    fn rolls_back_raw_rows_with_validation_error_after_append() {
        let mut buf = BytesMut::from(&b"prefix"[..]);

        append_raw_rows_with(&mut buf, |buf| {
            buf.extend_from_slice(&[TokenType::Row as u8, 0x01]);
            Ok(RawRowsAppend::new(vec![1]))
        })
        .expect_err("validation error should fail");

        assert_eq!(&b"prefix"[..], &buf[..]);
    }

    #[test]
    fn rejects_empty_raw_rows_with_append() {
        let mut buf = BytesMut::new();

        append_raw_rows_with(&mut buf, |_| Ok(RawRowsAppend::new(vec![0])))
            .expect_err("empty append should fail");

        assert!(buf.is_empty());
    }

    #[test]
    fn rejects_invalid_raw_rows_with_offsets_and_rolls_back() {
        let cases: &[(&[u8], &[usize])] = &[
            (&[TokenType::Row as u8], &[]),
            (&[0x00, TokenType::Row as u8], &[1]),
            (&[TokenType::Row as u8], &[0, 1]),
            (&[TokenType::Row as u8, 0x01], &[0, 0]),
            (&[TokenType::Row as u8, 0x01, 0x02], &[0, 1]),
        ];

        for (payload, row_token_offsets) in cases {
            let mut buf = BytesMut::from(&b"prefix"[..]);

            append_raw_rows_with(&mut buf, |buf| {
                buf.extend_from_slice(payload);
                Ok(RawRowsAppend::new(row_token_offsets.to_vec()))
            })
            .expect_err("invalid row offsets should fail");

            assert_eq!(&b"prefix"[..], &buf[..]);
        }
    }

    #[test]
    fn rejects_checked_batched_raw_rows_payload_with_empty_offsets() {
        let mut buf = BytesMut::new();

        append_raw_rows_payload_checked(&mut buf, &[TokenType::Row as u8], &[])
            .expect_err("empty offsets should fail");

        assert!(buf.is_empty());
    }

    #[test]
    fn rejects_checked_batched_raw_rows_payload_with_nonzero_first_offset() {
        let mut buf = BytesMut::new();

        append_raw_rows_payload_checked(&mut buf, &[0x00, TokenType::Row as u8], &[1])
            .expect_err("nonzero first offset should fail");

        assert!(buf.is_empty());
    }

    #[test]
    fn rejects_checked_batched_raw_rows_payload_with_out_of_bounds_offset() {
        let mut buf = BytesMut::new();

        append_raw_rows_payload_checked(&mut buf, &[TokenType::Row as u8], &[0, 1])
            .expect_err("out of bounds offset should fail");

        assert!(buf.is_empty());
    }

    #[test]
    fn rejects_checked_batched_raw_rows_payload_with_non_increasing_offsets() {
        let mut buf = BytesMut::new();

        append_raw_rows_payload_checked(&mut buf, &[TokenType::Row as u8, 0x01], &[0, 0])
            .expect_err("repeated offset should fail");

        assert!(buf.is_empty());
    }

    #[test]
    fn rejects_checked_batched_raw_rows_payload_with_offset_not_on_row_token() {
        let mut buf = BytesMut::new();

        append_raw_rows_payload_checked(&mut buf, &[TokenType::Row as u8, 0x01, 0x02], &[0, 1])
            .expect_err("offset not on row token should fail");

        assert!(buf.is_empty());
    }
}
