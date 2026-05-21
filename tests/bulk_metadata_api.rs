use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::io::{AsyncRead, AsyncWrite};
use tiberius::{
    BulkLoadColumn, BulkLoadColumns, BulkLoadConnectionWriteStats, BulkLoadDirectPacketWriteStats,
    BulkLoadPacketStats, BulkLoadRequest, BulkLoadStats, BulkLoadWriteTimingStats, Client,
    ColumnFlag, ColumnType, ExecuteResult, FixedLenType, TypeInfo, VarLenType,
};

struct ExternalStream;

impl AsyncRead for ExternalStream {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(Ok(0))
    }
}

impl AsyncWrite for ExternalStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[test]
fn external_crate_can_name_and_inspect_bulk_metadata_api() {
    fn inspect_columns(columns: &BulkLoadColumns<'_>) {
        let _column_count = columns.len();
        let _empty = columns.is_empty();

        for column in columns.iter() {
            inspect_column(column);
        }
    }

    fn inspect_request(req: &BulkLoadRequest<'_, ExternalStream>) {
        let columns = req.columns();
        let _column_count = columns.len();
        let _stats: BulkLoadPacketStats = req.packet_stats();
        let _timing: BulkLoadWriteTimingStats = req.write_timing_stats();
        let _all_stats: BulkLoadStats = req.stats();

        for column in columns {
            inspect_column(column);
        }
    }

    fn inspect_column(column: BulkLoadColumn<'_>) {
        let _ordinal: usize = column.ordinal();
        let _name: &str = column.name();
        let _column_type: ColumnType = column.column_type();
        let _nullable: bool = column.is_nullable();
        let _updateable: bool = column.is_updateable();
        let _flags_have_nullable = column.flags().contains(ColumnFlag::Nullable);

        match column.type_info() {
            TypeInfo::FixedLen(FixedLenType::Int4) => {}
            TypeInfo::VarLenSized(ctx) if ctx.r#type() == VarLenType::NVarchar => {
                let _len = ctx.len();
                let _collation_parts = ctx.collation().map(|collation| {
                    let _lcid = collation.lcid();
                    (collation.info(), collation.sort_id())
                });
            }
            TypeInfo::VarLenSizedPrecision {
                precision, scale, ..
            } => {
                let _numeric_shape = (*precision, *scale);
            }
            TypeInfo::Xml { size, .. } => {
                let _xml_size = *size;
            }
            _ => {}
        }
    }

    let _columns_type_check = inspect_columns as fn(&BulkLoadColumns<'_>);
    let _type_check = inspect_request as fn(&BulkLoadRequest<'_, ExternalStream>);
}

#[test]
fn external_crate_can_name_bulk_packet_stats_api() {
    let stats = BulkLoadPacketStats::default();

    let _write_calls: u64 = stats.write_packets_calls;
    let _packets: u64 = stats.packets_written;
    let _bytes: u64 = stats.packet_payload_bytes;
    let _max_payload: usize = stats.max_packet_payload_bytes;
    let _max_buffered: usize = stats.max_buffered_bytes_before_write;
    let _tail: usize = stats.buffered_bytes_after_last_write;
    let _final_payload: usize = stats.finalized_packet_payload_bytes;
}

#[test]
fn external_crate_can_name_bulk_write_timing_stats_api() {
    let stats = BulkLoadWriteTimingStats::default();

    let _write_packets_elapsed = stats.write_packets_elapsed;
    let _write_calls: u64 = stats.write_to_wire_calls;
    let _write_elapsed = stats.write_to_wire_elapsed;
    let _write_bytes: u64 = stats.write_to_wire_payload_bytes;
    let _max_write_elapsed = stats.max_write_to_wire_elapsed;
    let _max_write_bytes: usize = stats.max_write_to_wire_payload_bytes;
    let _flush_calls: u64 = stats.flush_calls;
    let _flush_elapsed = stats.flush_elapsed;
    let _max_flush_elapsed = stats.max_flush_elapsed;
    let _finalize_elapsed = stats.finalize_elapsed;
    let _finalize_write_elapsed = stats.finalize_write_to_wire_elapsed;
    let _finalize_flush_elapsed = stats.finalize_flush_elapsed;
    let _finalize_result_elapsed = stats.finalize_result_elapsed;
    let _connection_write: BulkLoadConnectionWriteStats = stats.connection_write;
    let _direct_packet_write: BulkLoadDirectPacketWriteStats = stats.direct_packet_write;
}

#[test]
fn external_crate_can_name_bulk_connection_write_stats_api() {
    let stats = BulkLoadConnectionWriteStats::default();

    let _calls: u64 = stats.calls;
    let _payload_bytes: u64 = stats.payload_bytes;
    let _ready_elapsed = stats.ready_elapsed;
    let _encode_elapsed = stats.encode_elapsed;
    let _flush_elapsed = stats.flush_elapsed;
    let _max_ready_elapsed = stats.max_ready_elapsed;
    let _max_encode_elapsed = stats.max_encode_elapsed;
    let _max_flush_elapsed = stats.max_flush_elapsed;
    let _max_payload_bytes: usize = stats.max_payload_bytes;
}

#[test]
fn external_crate_can_name_bulk_direct_packet_write_stats_api() {
    let stats = BulkLoadDirectPacketWriteStats::default();

    let _calls: u64 = stats.calls;
    let _payload_bytes: u64 = stats.payload_bytes;
    let _header_bytes: u64 = stats.header_bytes;
    let _write_calls: u64 = stats.write_calls;
    let _write_bytes: u64 = stats.write_bytes;
    let _max_write_bytes: usize = stats.max_write_bytes;
    let _write_elapsed = stats.write_elapsed;
    let _max_write_elapsed = stats.max_write_elapsed;
    let _flush_calls: u64 = stats.flush_calls;
    let _flush_elapsed = stats.flush_elapsed;
    let _max_flush_elapsed = stats.max_flush_elapsed;
}

#[test]
fn external_crate_can_name_combined_bulk_stats_api() {
    let stats = BulkLoadStats::default();

    let _packet: BulkLoadPacketStats = stats.packet;
    let _timing: BulkLoadWriteTimingStats = stats.write_timing;
}

#[test]
fn bulk_insert_accepts_table_string_shorter_than_request_lifetime() {
    async fn start_bulk_with_formatted_table<'client>(
        client: &'client mut Client<ExternalStream>,
        table_name: &str,
    ) -> tiberius::Result<BulkLoadRequest<'client, ExternalStream>> {
        let table_sql = format!("[dbo].[{table_name}]");

        client.bulk_insert(&table_sql).await
    }

    let _ = start_bulk_with_formatted_table;
}

#[test]
fn external_crate_can_call_split_bulk_insert_flow() {
    async fn split_bulk<'client>(
        client: &'client mut Client<ExternalStream>,
        table_name: &str,
    ) -> tiberius::Result<BulkLoadRequest<'client, ExternalStream>> {
        let table_sql = format!("[dbo].[{table_name}]");
        let columns = client.bulk_insert_columns(&table_sql).await?;

        client.bulk_insert_with_columns(&table_sql, columns).await
    }

    let _ = split_bulk;
}

#[test]
fn external_crate_can_call_raw_row_apis() {
    fn send_raw<'a>(
        req: &'a mut BulkLoadRequest<'a, ExternalStream>,
    ) -> impl Future<Output = tiberius::Result<()>> + 'a {
        async move {
            req.send_raw_row_payload([0x01, 0x02]).await?;
            req.send_raw_rows_payload([0xD1, 0x01, 0xD1, 0x02]).await?;
            req.send_raw_rows_payload_checked([0xD1, 0x01, 0xD1, 0x02], [0, 2])
                .await?;

            Ok(())
        }
    }

    let _type_check = send_raw;
}

#[test]
fn external_crate_can_enable_direct_packet_writes() {
    fn enable(req: &mut BulkLoadRequest<'_, ExternalStream>) {
        let _default_enabled: bool = req.direct_packet_writes_enabled();
        req.enable_direct_packet_writes();
        let _enabled: bool = req.direct_packet_writes_enabled();
    }

    let _type_check = enable;
}

#[test]
fn external_crate_can_finalize_with_packet_stats() {
    async fn finish_with_stats(
        req: BulkLoadRequest<'_, ExternalStream>,
    ) -> tiberius::Result<(ExecuteResult, BulkLoadPacketStats)> {
        req.finalize_with_packet_stats().await
    }

    let _type_check = finish_with_stats;
}

#[test]
fn external_crate_can_finalize_with_combined_bulk_stats() {
    async fn finish_with_stats(
        req: BulkLoadRequest<'_, ExternalStream>,
    ) -> tiberius::Result<(ExecuteResult, BulkLoadStats)> {
        req.finalize_with_stats().await
    }

    let _type_check = finish_with_stats;
}
