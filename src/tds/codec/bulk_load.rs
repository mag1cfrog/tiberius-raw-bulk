use asynchronous_codec::BytesMut;
use enumflags2::BitFlags;
use futures_util::io::{AsyncRead, AsyncWrite};
use tracing::{event, Level};

use crate::{
    client::Connection, sql_read_bytes::SqlReadBytes, BytesMutWithDataColumns, ColumnFlag,
    ColumnType, ExecuteResult,
};

use super::{
    Encode, MetaDataColumn, PacketHeader, PacketStatus, TokenColMetaData, TokenDone, TokenRow,
    TypeInfo, HEADER_BYTES,
};

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
        self.columns
            .iter()
            .enumerate()
            .map(|(ordinal, column)| BulkLoadColumn { ordinal, column })
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

    /// Ends the bulk load, flushing all pending data to the wire.
    ///
    /// This method must be called after sending all the data to flush all
    /// pending data and to get the server actually to store the rows to the
    /// table.
    pub async fn finalize(mut self) -> crate::Result<ExecuteResult> {
        TokenDone::default().encode(&mut self.buf)?;
        self.write_packets().await?;

        let mut header = PacketHeader::bulk_load(self.packet_id);
        header.set_status(PacketStatus::EndOfMessage);

        let data = self.buf.split();

        event!(
            Level::TRACE,
            "Finalizing a bulk insert ({} bytes)",
            data.len() + HEADER_BYTES,
        );

        self.connection.write_to_wire(header, data).await?;
        self.connection.flush_sink().await?;

        ExecuteResult::new(self.connection).await
    }

    async fn write_packets(&mut self) -> crate::Result<()> {
        let packet_size = (self.connection.context().packet_size() as usize) - HEADER_BYTES;

        while self.buf.len() > packet_size {
            let header = PacketHeader::bulk_load(self.packet_id);
            let data = self.buf.split_to(packet_size);

            event!(
                Level::TRACE,
                "Bulk insert packet ({} bytes)",
                data.len() + HEADER_BYTES,
            );

            self.connection.write_to_wire(header, data).await?;
        }

        Ok(())
    }
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
}
