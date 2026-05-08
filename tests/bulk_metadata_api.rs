use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::io::{AsyncRead, AsyncWrite};
use tiberius::{
    BulkLoadColumn, BulkLoadRequest, ColumnFlag, ColumnType, FixedLenType, TypeInfo, VarLenType,
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
    fn inspect_request(req: &BulkLoadRequest<'_, ExternalStream>) {
        let columns = req.columns();
        let _column_count = columns.len();

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

    let _type_check = inspect_request as fn(&BulkLoadRequest<'_, ExternalStream>);
}
