use asynchronous_codec::BytesMut;
use bytes::BufMut;
use enumflags2::BitFlags;
use futures_util::io::{AsyncRead, AsyncWrite};
use tracing::{event, Level};

use crate::{
    client::Connection, sql_read_bytes::SqlReadBytes, BytesMutWithDataColumns, ColumnFlag,
    ColumnType, ExecuteResult,
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
        bulk_load_columns(&self.columns)
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
