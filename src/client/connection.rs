#[cfg(all(windows, feature = "winauth"))]
use super::sspi::SspiClient;
#[cfg(any(
    feature = "rustls",
    feature = "native-tls",
    feature = "vendored-openssl"
))]
use crate::client::{tls::TlsPreloginWrapper, tls_stream::create_tls_stream};
use crate::{
    client::{tls::MaybeTlsStream, AuthMethod, Config},
    observability,
    tds::{
        codec::{
            self, Encode, LoginMessage, Packet, PacketCodec, PacketHeader, PacketStatus,
            PreloginMessage, TokenDone,
        },
        stream::TokenStream,
        Context, HEADER_BYTES,
    },
    EncryptionLevel, SqlReadBytes,
};
use asynchronous_codec::Framed;
use bytes::BytesMut;
#[cfg(any(windows, feature = "integrated-auth-gssapi"))]
use codec::TokenSspi;
use futures_util::future::poll_fn;
use futures_util::io::{AsyncRead, AsyncWrite};
use futures_util::ready;
#[cfg(feature = "bulk-load-profile")]
use futures_util::sink::Sink;
use futures_util::sink::SinkExt;
use futures_util::stream::{Stream, TryStream, TryStreamExt};
#[cfg(all(unix, feature = "integrated-auth-gssapi"))]
use libgssapi::{
    context::{ClientCtx, CtxFlags},
    credential::{Cred, CredUsage},
    name::Name,
    oid::{OidSet, GSS_MECH_KRB5, GSS_NT_KRB5_PRINCIPAL},
};
use pretty_hex::*;
#[cfg(all(unix, feature = "integrated-auth-gssapi"))]
use std::ops::Deref;
use std::{cmp, fmt::Debug, io, pin::Pin, task, time::Instant};
use task::Poll;
use tracing::{event, Instrument, Level};

/// A `Connection` is an abstraction between the [`Client`] and the server. It
/// can be used as a `Stream` to fetch [`Packet`]s from and to `send` packets
/// splitting them to the negotiated limit automatically.
///
/// `Connection` is not meant to use directly, but as an abstraction layer for
/// the numerous `Stream`s for easy packet handling.
///
/// [`Client`]: struct.Encode.html
/// [`Packet`]: ../protocol/codec/struct.Packet.html
pub(crate) struct Connection<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    transport: Framed<MaybeTlsStream<S>, PacketCodec>,
    flushed: bool,
    context: Context,
    buf: BytesMut,
}

#[cfg(feature = "bulk-load-profile")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ConnectionWriteTiming {
    pub(crate) ready_elapsed: std::time::Duration,
    pub(crate) encode_elapsed: std::time::Duration,
    pub(crate) flush_elapsed: std::time::Duration,
}

#[cfg(feature = "bulk-load-profile")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DirectPacketWriteTiming {
    pub(crate) header_bytes: usize,
    pub(crate) payload_bytes: usize,
    pub(crate) raw_stream: bool,
    pub(crate) tls_stream: bool,
    pub(crate) write_calls: u64,
    pub(crate) write_bytes: u64,
    pub(crate) max_write_bytes: usize,
    pub(crate) write_elapsed: std::time::Duration,
    pub(crate) max_write_elapsed: std::time::Duration,
    pub(crate) header_write_calls: u64,
    pub(crate) header_write_bytes: u64,
    pub(crate) header_max_write_bytes: usize,
    pub(crate) header_write_elapsed: std::time::Duration,
    pub(crate) header_max_write_elapsed: std::time::Duration,
    pub(crate) header_partial_writes: u64,
    pub(crate) payload_write_calls: u64,
    pub(crate) payload_write_bytes: u64,
    pub(crate) payload_max_write_bytes: usize,
    pub(crate) payload_write_elapsed: std::time::Duration,
    pub(crate) payload_max_write_elapsed: std::time::Duration,
    pub(crate) payload_partial_writes: u64,
    pub(crate) poll_write_polls: u64,
    pub(crate) poll_write_pending_count: u64,
    pub(crate) poll_write_pending_elapsed: std::time::Duration,
    pub(crate) poll_write_max_pending_elapsed: std::time::Duration,
    pub(crate) poll_write_ready_count: u64,
    pub(crate) poll_write_ready_elapsed: std::time::Duration,
    pub(crate) poll_write_max_ready_elapsed: std::time::Duration,
    pub(crate) flush_elapsed: std::time::Duration,
    pub(crate) flush_pending_count: u64,
    pub(crate) flush_pending_elapsed: std::time::Duration,
    pub(crate) flush_max_pending_elapsed: std::time::Duration,
}

#[cfg(feature = "bulk-load-profile")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DirectPacketPollWriteSummary {
    pub(crate) polls: u64,
    pub(crate) pending_count: u64,
    pub(crate) pending_elapsed: std::time::Duration,
    pub(crate) max_pending_elapsed: std::time::Duration,
    pub(crate) ready_count: u64,
    pub(crate) ready_elapsed: std::time::Duration,
    pub(crate) max_ready_elapsed: std::time::Duration,
}

#[cfg(feature = "bulk-load-profile")]
impl DirectPacketWriteTiming {
    pub(crate) fn poll_write_summary(self) -> DirectPacketPollWriteSummary {
        DirectPacketPollWriteSummary {
            polls: self.poll_write_polls,
            pending_count: self.poll_write_pending_count,
            pending_elapsed: self.poll_write_pending_elapsed,
            max_pending_elapsed: self.poll_write_max_pending_elapsed,
            ready_count: self.poll_write_ready_count,
            ready_elapsed: self.poll_write_ready_elapsed,
            max_ready_elapsed: self.poll_write_max_ready_elapsed,
        }
    }
}

#[cfg(feature = "bulk-load-profile")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectPacketWritePart {
    Header,
    Payload,
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> Debug for Connection<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("transport", &"Framed<..>")
            .field("flushed", &self.flushed)
            .field("context", &self.context)
            .field("buf", &self.buf.as_ref().hex_dump())
            .finish()
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> Connection<S> {
    /// Creates a new connection
    pub(crate) async fn connect(config: Config, tcp_stream: S) -> crate::Result<Connection<S>> {
        let requested_encryption = config.encryption;
        let fed_auth_required = matches!(config.auth, AuthMethod::AADToken(_));
        let span = observability::connection_connect_span(requested_encryption, fed_auth_required);

        async move {
            let connect_start = Instant::now();
            observability::emit_connection_setup_start(requested_encryption, fed_auth_required);

            let result =
                Self::connect_inner(config, tcp_stream, fed_auth_required, connect_start).await;

            if let Err(error) = &result {
                observability::emit_connection_setup_failed(connect_start.elapsed(), error);
            }

            result
        }
        .instrument(span)
        .await
    }

    async fn connect_inner(
        config: Config,
        tcp_stream: S,
        fed_auth_required: bool,
        connect_start: Instant,
    ) -> crate::Result<Connection<S>> {
        let context = {
            let mut context = Context::new();
            context.set_spn(config.get_host(), config.get_port());
            context
        };

        let transport = Framed::new(MaybeTlsStream::Raw(tcp_stream), PacketCodec);

        let mut connection = Self {
            transport,
            context,
            flushed: false,
            buf: BytesMut::new(),
        };

        let prelogin = connection
            .prelogin(config.encryption, fed_auth_required)
            .await?;

        let encryption = prelogin.negotiated_encryption(config.encryption);

        let connection = connection.tls_handshake(&config, encryption).await?;

        let auth_method = observability::auth_method_category(&config.auth);
        let login_span = observability::login_flow_span(encryption, auth_method);
        let login_start = Instant::now();
        let connection = async move {
            observability::emit_login_flow_start(encryption, auth_method);

            let result = async {
                let mut connection = connection
                    .login(
                        config.auth,
                        encryption,
                        config.database,
                        config.host,
                        config.application_name,
                        config.packet_size,
                        config.readonly,
                        prelogin,
                    )
                    .await?;

                connection.flush_done().await?;
                Ok(connection)
            }
            .await;

            match &result {
                Ok(_) => observability::emit_login_flow_completed(
                    login_start.elapsed(),
                    encryption,
                    auth_method,
                ),
                Err(error) => observability::emit_login_flow_failed(
                    login_start.elapsed(),
                    encryption,
                    auth_method,
                    error,
                ),
            }

            result
        }
        .instrument(login_span)
        .await?;

        observability::emit_connection_setup_completed(
            connect_start.elapsed(),
            encryption,
            connection.context.packet_size(),
        );

        Ok(connection)
    }

    /// Flush the incoming token stream until receiving `DONE` token.
    async fn flush_done(&mut self) -> crate::Result<TokenDone> {
        TokenStream::new(self).flush_done().await
    }

    #[cfg(any(windows, feature = "integrated-auth-gssapi"))]
    /// Flush the incoming token stream until receiving `SSPI` token.
    async fn flush_sspi(&mut self) -> crate::Result<TokenSspi> {
        TokenStream::new(self).flush_sspi().await
    }

    #[cfg(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    ))]
    fn post_login_encryption(mut self, encryption: EncryptionLevel) -> Self {
        if let EncryptionLevel::Off = encryption {
            observability::emit_tls_post_login_downgraded(encryption);

            let Self { transport, .. } = self;
            let tcp = transport.into_inner().into_inner();
            self.transport = Framed::new(MaybeTlsStream::Raw(tcp), PacketCodec);
        }

        self
    }

    #[cfg(not(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    )))]
    fn post_login_encryption(self, _: EncryptionLevel) -> Self {
        self
    }

    /// Send an item to the wire. Header should define the item type and item should implement
    /// [`Encode`], defining the byte structure for the wire.
    ///
    /// The `send` will split the packet into multiple packets if bigger than
    /// the negotiated packet size, and handle flushing to the wire in an optimal way.
    ///
    /// [`Encode`]: ../protocol/codec/trait.Encode.html
    pub async fn send<E>(&mut self, mut header: PacketHeader, item: E) -> crate::Result<()>
    where
        E: Sized + Encode<BytesMut>,
    {
        self.flushed = false;
        let packet_size = (self.context.packet_size() as usize) - HEADER_BYTES;

        let mut payload = BytesMut::new();
        item.encode(&mut payload)?;

        while !payload.is_empty() {
            let writable = cmp::min(payload.len(), packet_size);
            let split_payload = payload.split_to(writable);

            if payload.is_empty() {
                header.set_status(PacketStatus::EndOfMessage);
            } else {
                header.set_status(PacketStatus::NormalMessage);
            }

            event!(
                Level::TRACE,
                "Sending a packet ({} bytes)",
                split_payload.len() + HEADER_BYTES,
            );

            self.write_to_wire(header, split_payload).await?;
        }

        self.flush_sink().await?;

        Ok(())
    }

    /// Sends a packet of data to the database.
    ///
    /// # Warning
    ///
    /// Please be sure the packet size doesn't exceed the largest allowed size
    /// dictaded by the server.
    pub(crate) async fn write_to_wire(
        &mut self,
        header: PacketHeader,
        data: BytesMut,
    ) -> crate::Result<()> {
        self.flushed = false;

        let packet = Packet::new(header, data);
        self.transport.send(packet).await?;

        Ok(())
    }

    /// Sends a packet and reports the framed sink phases hidden by
    /// [`SinkExt::send`].
    ///
    /// This is used by bulk-load benchmarks only. It intentionally mirrors the
    /// `send` sequence of readiness, encoding, and flush so the measured path
    /// stays behavior-equivalent to [`write_to_wire`].
    #[cfg(feature = "bulk-load-profile")]
    pub(crate) async fn write_to_wire_with_timing(
        &mut self,
        header: PacketHeader,
        data: BytesMut,
    ) -> crate::Result<ConnectionWriteTiming> {
        self.flushed = false;

        let packet = Packet::new(header, data);
        let ready_start = std::time::Instant::now();
        poll_fn(|cx| Pin::new(&mut self.transport).poll_ready(cx)).await?;
        let ready_elapsed = ready_start.elapsed();

        let encode_start = std::time::Instant::now();
        Pin::new(&mut self.transport).start_send(packet)?;
        let encode_elapsed = encode_start.elapsed();

        let flush_start = std::time::Instant::now();
        poll_fn(|cx| Pin::new(&mut self.transport).poll_flush(cx)).await?;
        let flush_elapsed = flush_start.elapsed();

        Ok(ConnectionWriteTiming {
            ready_elapsed,
            encode_elapsed,
            flush_elapsed,
        })
    }

    /// Writes one complete direct TDS packet and returns low-level write timing.
    ///
    /// `packet` must already include the 8-byte TDS header followed by the
    /// packet payload. Writing the contiguous buffer keeps direct bulk writes
    /// equivalent to the framed packet path and avoids a separate tiny header
    /// write for every bulk packet.
    #[cfg(feature = "bulk-load-profile")]
    pub(crate) async fn write_direct_packet_buffer_with_timing(
        &mut self,
        packet: &[u8],
    ) -> crate::Result<DirectPacketWriteTiming> {
        self.flushed = false;

        if packet.len() < HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "direct TDS packet buffer is shorter than the packet header",
            )
            .into());
        }

        let mut timing = DirectPacketWriteTiming {
            header_bytes: HEADER_BYTES,
            payload_bytes: packet.len() - HEADER_BYTES,
            raw_stream: matches!(&*self.transport, MaybeTlsStream::Raw(_)),
            #[cfg(any(
                feature = "rustls",
                feature = "native-tls",
                feature = "vendored-openssl"
            ))]
            tls_stream: matches!(&*self.transport, MaybeTlsStream::Tls(_)),
            ..DirectPacketWriteTiming::default()
        };

        Self::write_all_direct_packet_contiguous(&mut self.transport, packet, &mut timing).await?;

        let flush_start = std::time::Instant::now();
        Self::poll_direct_packet_flush(&mut self.transport, &mut timing).await?;
        timing.flush_elapsed = flush_start.elapsed();

        Ok(timing)
    }

    /// Writes one complete direct TDS packet without collecting profiling data.
    ///
    /// `packet` must already include the 8-byte TDS header followed by the
    /// packet payload.
    #[cfg(not(feature = "bulk-load-profile"))]
    pub(crate) async fn write_direct_packet_buffer(&mut self, packet: &[u8]) -> crate::Result<()> {
        self.flushed = false;

        if packet.len() < HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "direct TDS packet buffer is shorter than the packet header",
            )
            .into());
        }

        Self::write_all_direct_packet_contiguous_plain(&mut self.transport, packet).await?;
        poll_fn(|cx| Pin::new(&mut *self.transport).poll_flush(cx)).await?;

        Ok(())
    }

    #[cfg(all(test, feature = "bulk-load-profile"))]
    async fn write_all_direct_packet_bytes(
        stream: &mut MaybeTlsStream<S>,
        mut bytes: &[u8],
        part: DirectPacketWritePart,
        timing: &mut DirectPacketWriteTiming,
    ) -> crate::Result<()> {
        while !bytes.is_empty() {
            let remaining = bytes.len();
            let write_start = std::time::Instant::now();
            let written = Self::poll_direct_packet_write(stream, bytes, timing).await?;
            let elapsed = write_start.elapsed();

            if written == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "failed to write direct TDS packet bytes",
                )
                .into());
            }

            timing.write_calls = timing.write_calls.saturating_add(1);
            timing.write_bytes = timing
                .write_bytes
                .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
            timing.max_write_bytes = timing.max_write_bytes.max(written);
            timing.write_elapsed += elapsed;
            timing.max_write_elapsed = timing.max_write_elapsed.max(elapsed);
            Self::record_direct_packet_write_part(timing, part, remaining, written, elapsed);
            bytes = &bytes[written..];
        }

        Ok(())
    }

    #[cfg(not(feature = "bulk-load-profile"))]
    async fn write_all_direct_packet_contiguous_plain(
        stream: &mut MaybeTlsStream<S>,
        packet: &[u8],
    ) -> crate::Result<()> {
        let mut written_total = 0;
        while written_total < packet.len() {
            let written =
                poll_fn(|cx| Pin::new(&mut *stream).poll_write(cx, &packet[written_total..]))
                    .await?;

            if written == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "failed to write direct TDS packet bytes",
                )
                .into());
            }

            written_total = written_total.saturating_add(written);
        }

        Ok(())
    }

    #[cfg(feature = "bulk-load-profile")]
    async fn write_all_direct_packet_contiguous(
        stream: &mut MaybeTlsStream<S>,
        packet: &[u8],
        timing: &mut DirectPacketWriteTiming,
    ) -> crate::Result<()> {
        let payload_len = packet.len().saturating_sub(HEADER_BYTES);
        let mut written_total = 0;
        while written_total < packet.len() {
            let write_start_offset = written_total;
            let write_start = std::time::Instant::now();
            let written =
                Self::poll_direct_packet_write(stream, &packet[write_start_offset..], timing)
                    .await?;
            let elapsed = write_start.elapsed();

            if written == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "failed to write direct TDS packet bytes",
                )
                .into());
            }

            timing.write_calls = timing.write_calls.saturating_add(1);
            timing.write_bytes = timing
                .write_bytes
                .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
            timing.max_write_bytes = timing.max_write_bytes.max(written);
            timing.write_elapsed += elapsed;
            timing.max_write_elapsed = timing.max_write_elapsed.max(elapsed);

            let write_end_offset = write_start_offset.saturating_add(written);
            if write_end_offset > packet.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "direct TDS packet contiguous write reported too many bytes",
                )
                .into());
            }

            let header_written = write_end_offset
                .min(HEADER_BYTES)
                .saturating_sub(write_start_offset.min(HEADER_BYTES));
            let payload_start_offset = HEADER_BYTES;
            let payload_end_offset = HEADER_BYTES.saturating_add(payload_len);
            let payload_written = write_end_offset.min(payload_end_offset).saturating_sub(
                write_start_offset
                    .max(payload_start_offset)
                    .min(payload_end_offset),
            );
            // A single raw write can cover both header and payload bytes. Keep
            // the public timing split by attributing the elapsed time to the
            // payload side in that case, so header plus payload elapsed does
            // not double-count the same write.
            if header_written > 0 {
                let header_remaining =
                    HEADER_BYTES.saturating_sub(write_start_offset.min(HEADER_BYTES));
                let header_elapsed = if payload_written > 0 {
                    std::time::Duration::ZERO
                } else {
                    elapsed
                };
                Self::record_direct_packet_write_part(
                    timing,
                    DirectPacketWritePart::Header,
                    header_remaining,
                    header_written,
                    header_elapsed,
                );
            }
            if payload_written > 0 {
                let payload_remaining = payload_len.saturating_sub(
                    write_start_offset
                        .saturating_sub(payload_start_offset)
                        .min(payload_len),
                );
                Self::record_direct_packet_write_part(
                    timing,
                    DirectPacketWritePart::Payload,
                    payload_remaining,
                    payload_written,
                    elapsed,
                );
            }

            written_total = write_end_offset;
        }

        Ok(())
    }

    #[cfg(feature = "bulk-load-profile")]
    async fn poll_direct_packet_write(
        stream: &mut MaybeTlsStream<S>,
        bytes: &[u8],
        timing: &mut DirectPacketWriteTiming,
    ) -> io::Result<usize> {
        let mut pending_start = None;

        poll_fn(|cx| {
            timing.poll_write_polls = timing.poll_write_polls.saturating_add(1);
            let ready_start = std::time::Instant::now();
            match Pin::new(&mut *stream).poll_write(cx, bytes) {
                Poll::Pending => {
                    timing.poll_write_pending_count =
                        timing.poll_write_pending_count.saturating_add(1);
                    if pending_start.is_none() {
                        pending_start = Some(std::time::Instant::now());
                    }
                    Poll::Pending
                }
                Poll::Ready(result) => {
                    let ready_elapsed = ready_start.elapsed();
                    timing.poll_write_ready_count = timing.poll_write_ready_count.saturating_add(1);
                    timing.poll_write_ready_elapsed += ready_elapsed;
                    timing.poll_write_max_ready_elapsed =
                        timing.poll_write_max_ready_elapsed.max(ready_elapsed);

                    if let Some(start) = pending_start.take() {
                        let pending_elapsed = start.elapsed();
                        timing.poll_write_pending_elapsed += pending_elapsed;
                        timing.poll_write_max_pending_elapsed =
                            timing.poll_write_max_pending_elapsed.max(pending_elapsed);
                    }

                    Poll::Ready(result)
                }
            }
        })
        .await
    }

    #[cfg(feature = "bulk-load-profile")]
    async fn poll_direct_packet_flush(
        stream: &mut MaybeTlsStream<S>,
        timing: &mut DirectPacketWriteTiming,
    ) -> io::Result<()> {
        let mut pending_start = None;

        poll_fn(|cx| match Pin::new(&mut *stream).poll_flush(cx) {
            Poll::Pending => {
                timing.flush_pending_count = timing.flush_pending_count.saturating_add(1);
                if pending_start.is_none() {
                    pending_start = Some(std::time::Instant::now());
                }
                Poll::Pending
            }
            Poll::Ready(result) => {
                if let Some(start) = pending_start.take() {
                    let pending_elapsed = start.elapsed();
                    timing.flush_pending_elapsed += pending_elapsed;
                    timing.flush_max_pending_elapsed =
                        timing.flush_max_pending_elapsed.max(pending_elapsed);
                }

                Poll::Ready(result)
            }
        })
        .await
    }

    #[cfg(feature = "bulk-load-profile")]
    fn record_direct_packet_write_part(
        timing: &mut DirectPacketWriteTiming,
        part: DirectPacketWritePart,
        remaining: usize,
        written: usize,
        elapsed: std::time::Duration,
    ) {
        let partial_write = u64::from(written < remaining);

        match part {
            DirectPacketWritePart::Header => {
                timing.header_write_calls = timing.header_write_calls.saturating_add(1);
                timing.header_write_bytes = timing
                    .header_write_bytes
                    .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
                timing.header_max_write_bytes = timing.header_max_write_bytes.max(written);
                timing.header_write_elapsed += elapsed;
                timing.header_max_write_elapsed = timing.header_max_write_elapsed.max(elapsed);
                timing.header_partial_writes =
                    timing.header_partial_writes.saturating_add(partial_write);
            }
            DirectPacketWritePart::Payload => {
                timing.payload_write_calls = timing.payload_write_calls.saturating_add(1);
                timing.payload_write_bytes = timing
                    .payload_write_bytes
                    .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
                timing.payload_max_write_bytes = timing.payload_max_write_bytes.max(written);
                timing.payload_write_elapsed += elapsed;
                timing.payload_max_write_elapsed = timing.payload_max_write_elapsed.max(elapsed);
                timing.payload_partial_writes =
                    timing.payload_partial_writes.saturating_add(partial_write);
            }
        }
    }

    /// Sends all pending packages to the wire.
    pub(crate) async fn flush_sink(&mut self) -> crate::Result<()> {
        self.transport.flush().await
    }

    /// Cleans the packet stream from previous use. It is important to use the
    /// whole stream before using the connection again. Flushing the stream
    /// makes sure we don't have any old data causing undefined behaviour after
    /// previous queries.
    ///
    /// Calling this will slow down the queries if stream is still dirty if all
    /// results are not handled.
    pub async fn flush_stream(&mut self) -> crate::Result<()> {
        self.buf.truncate(0);

        if self.flushed {
            return Ok(());
        }

        while let Some(packet) = self.try_next().await? {
            event!(
                Level::WARN,
                "Flushing unhandled packet from the wire. Please consume your streams!",
            );

            let is_last = packet.is_last();

            if is_last {
                break;
            }
        }

        Ok(())
    }

    /// True if the underlying stream has no more data and is consumed
    /// completely.
    pub fn is_eof(&self) -> bool {
        self.flushed && self.buf.is_empty()
    }

    /// A message sent by the client to set up context for login. The server
    /// responds to a client PRELOGIN message with a message of packet header
    /// type 0x04 and with the packet data containing a PRELOGIN structure.
    ///
    /// This message stream is also used to wrap the TLS handshake payload if
    /// encryption is needed. In this scenario, where PRELOGIN message is
    /// transporting the TLS handshake payload, the packet data is simply the
    /// raw bytes of the TLS handshake payload.
    async fn prelogin(
        &mut self,
        encryption: EncryptionLevel,
        fed_auth_required: bool,
    ) -> crate::Result<PreloginMessage> {
        let prelogin_start = Instant::now();
        observability::emit_prelogin_start(encryption, fed_auth_required);

        let result = async {
            let mut msg = PreloginMessage::new();
            msg.encryption = encryption;
            msg.fed_auth_required = fed_auth_required;

            let id = self.context.next_packet_id();
            self.send(PacketHeader::pre_login(id), msg).await?;

            let response: PreloginMessage = codec::collect_from(self).await?;
            // threadid (should be empty when sent from server to client)
            debug_assert_eq!(response.thread_id, 0);
            Ok(response)
        }
        .await;

        match &result {
            Ok(response) => observability::emit_prelogin_completed(
                prelogin_start.elapsed(),
                response.encryption,
                response.fed_auth_required,
            ),
            Err(error) => observability::emit_prelogin_failed(prelogin_start.elapsed(), error),
        }

        result
    }

    /// Defines the login record rules with SQL Server. Authentication with
    /// connection options.
    #[allow(clippy::too_many_arguments)]
    async fn login<'a>(
        mut self,
        auth: AuthMethod,
        encryption: EncryptionLevel,
        db: Option<String>,
        server_name: Option<String>,
        application_name: Option<String>,
        packet_size: Option<u32>,
        readonly: bool,
        prelogin: PreloginMessage,
    ) -> crate::Result<Self> {
        let mut login_message = LoginMessage::new();

        if let Some(packet_size) = packet_size {
            login_message.packet_size(packet_size);
        }

        if let Some(db) = db {
            login_message.db_name(db);
        }

        if let Some(server_name) = server_name {
            login_message.server_name(server_name);
        }

        if let Some(app_name) = application_name {
            login_message.app_name(app_name);
        }

        login_message.readonly(readonly);

        match auth {
            #[cfg(all(windows, feature = "winauth"))]
            AuthMethod::Integrated => {
                let mut client = SspiClient::integrated(self.context.spn())?;

                login_message.integrated_security(client.next_bytes(None)?);

                let id = self.context.next_packet_id();
                self.send(PacketHeader::login(id), login_message).await?;

                self = self.post_login_encryption(encryption);

                let sspi_bytes = self.flush_sspi().await?;

                match client.next_bytes(Some(sspi_bytes.as_ref()))? {
                    Some(sspi_response) => {
                        let id = self.context.next_packet_id();
                        let header = PacketHeader::login(id);

                        let token = TokenSspi::new(sspi_response);
                        self.send(header, token).await?;
                    }
                    None => unreachable!(),
                }
            }
            #[cfg(all(unix, feature = "integrated-auth-gssapi"))]
            AuthMethod::Integrated => {
                let mut s = OidSet::new()?;
                s.add(&GSS_MECH_KRB5)?;

                let client_cred = Cred::acquire(None, None, CredUsage::Initiate, Some(&s))?;

                let mut ctx = ClientCtx::new(
                    Some(client_cred),
                    Name::new(self.context.spn().as_bytes(), Some(&GSS_NT_KRB5_PRINCIPAL))?,
                    CtxFlags::GSS_C_MUTUAL_FLAG | CtxFlags::GSS_C_SEQUENCE_FLAG,
                    None,
                );

                let init_token = ctx.step(None, None)?;

                login_message.integrated_security(Some(Vec::from(init_token.unwrap().deref())));

                let id = self.context.next_packet_id();
                self.send(PacketHeader::login(id), login_message).await?;

                self = self.post_login_encryption(encryption);

                let auth_bytes = self.flush_sspi().await?;

                let next_token = match ctx.step(Some(auth_bytes.as_ref()), None)? {
                    Some(response) => TokenSspi::new(Vec::from(response.deref())),
                    None => TokenSspi::new(Vec::new()),
                };

                let id = self.context.next_packet_id();
                let header = PacketHeader::login(id);

                self.send(header, next_token).await?;
            }
            #[cfg(all(windows, feature = "winauth"))]
            AuthMethod::Windows(auth) => {
                let spn = self.context.spn().to_string();
                let mut client =
                    SspiClient::with_credentials(&spn, auth.domain, auth.user, auth.password)?;

                login_message.integrated_security(client.next_bytes(None)?);

                let id = self.context.next_packet_id();
                self.send(PacketHeader::login(id), login_message).await?;

                self = self.post_login_encryption(encryption);

                let sspi_bytes = self.flush_sspi().await?;

                match client.next_bytes(Some(sspi_bytes.as_ref()))? {
                    Some(sspi_response) => {
                        let id = self.context.next_packet_id();
                        let header = PacketHeader::login(id);

                        let token = TokenSspi::new(sspi_response);
                        self.send(header, token).await?;
                    }
                    None => unreachable!(),
                }
            }
            AuthMethod::None => {
                let id = self.context.next_packet_id();
                self.send(PacketHeader::login(id), login_message).await?;
                self = self.post_login_encryption(encryption);
            }
            AuthMethod::SqlServer(auth) => {
                login_message.user_name(auth.user());
                login_message.password(auth.password());

                let id = self.context.next_packet_id();
                self.send(PacketHeader::login(id), login_message).await?;
                self = self.post_login_encryption(encryption);
            }
            AuthMethod::AADToken(token) => {
                login_message.aad_token(token, prelogin.fed_auth_required, prelogin.nonce);
                let id = self.context.next_packet_id();
                self.send(PacketHeader::login(id), login_message).await?;
                self = self.post_login_encryption(encryption);
            }
        }

        Ok(self)
    }

    /// Implements the TLS handshake with the SQL Server.
    #[cfg(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    ))]
    async fn tls_handshake(
        self,
        config: &Config,
        encryption: EncryptionLevel,
    ) -> crate::Result<Self> {
        let tls_backend = observability::tls_backend_name();
        let tls_span = observability::tls_negotiation_span(encryption, tls_backend);
        let tls_start = Instant::now();

        async move {
            observability::emit_tls_negotiation_start(encryption, tls_backend);

            let result = async {
                if encryption != EncryptionLevel::NotSupported {
                    let Self {
                        transport, context, ..
                    } = self;
                    let mut stream = match transport.into_inner() {
                        MaybeTlsStream::Raw(tcp) => {
                            create_tls_stream(config, TlsPreloginWrapper::new(tcp)).await?
                        }
                        _ => unreachable!(),
                    };

                    stream.get_mut().handshake_complete();

                    let transport = Framed::new(MaybeTlsStream::Tls(stream), PacketCodec);

                    Ok((
                        Self {
                            transport,
                            context,
                            flushed: false,
                            buf: BytesMut::new(),
                        },
                        true,
                    ))
                } else {
                    Ok((self, false))
                }
            }
            .await;

            match result {
                Ok((connection, tls_used)) => {
                    observability::emit_tls_negotiation_completed(
                        tls_start.elapsed(),
                        encryption,
                        tls_backend,
                        tls_used,
                    );

                    Ok(connection)
                }
                Err(error) => {
                    observability::emit_tls_negotiation_failed(
                        tls_start.elapsed(),
                        encryption,
                        tls_backend,
                        &error,
                    );

                    Err(error)
                }
            }
        }
        .instrument(tls_span)
        .await
    }

    /// Implements the TLS handshake with the SQL Server.
    #[cfg(not(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    )))]
    async fn tls_handshake(self, _: &Config, _: EncryptionLevel) -> crate::Result<Self> {
        let tls_backend = observability::tls_backend_name();
        let encryption = EncryptionLevel::NotSupported;
        let tls_span = observability::tls_negotiation_span(encryption, tls_backend);
        let tls_start = Instant::now();

        async move {
            observability::emit_tls_negotiation_start(encryption, tls_backend);
            observability::emit_tls_negotiation_completed(
                tls_start.elapsed(),
                encryption,
                tls_backend,
                false,
            );

            Ok(self)
        }
        .instrument(tls_span)
        .await
    }

    pub(crate) async fn close(mut self) -> crate::Result<()> {
        self.transport.close().await
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> Stream for Connection<S> {
    type Item = crate::Result<Packet>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        match ready!(this.transport.try_poll_next_unpin(cx)) {
            Some(Ok(packet)) => {
                this.flushed = packet.is_last();
                Poll::Ready(Some(Ok(packet)))
            }
            Some(Err(e)) => Poll::Ready(Some(Err(e))),
            None => Poll::Ready(None),
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> futures_util::io::AsyncRead for Connection<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut this = self.get_mut();
        let size = buf.len();

        if this.buf.len() < size {
            while let Some(item) = ready!(Pin::new(&mut this).try_poll_next(cx)) {
                match item {
                    Ok(packet) => {
                        let (_, payload) = packet.into_parts();
                        this.buf.extend(payload);

                        if this.buf.len() >= size {
                            break;
                        }
                    }
                    Err(e) => {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            e.to_string(),
                        )))
                    }
                }
            }

            // Got EOF before having all the data.
            if this.buf.len() < size {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "No more packets in the wire",
                )));
            }
        }

        buf.copy_from_slice(this.buf.split_to(size).as_ref());
        Poll::Ready(Ok(size))
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> SqlReadBytes for Connection<S> {
    /// Hex dump of the current buffer.
    fn debug_buffer(&self) {
        dbg!(self.buf.as_ref().hex_dump());
    }

    /// The current execution context.
    fn context(&self) -> &Context {
        &self.context
    }

    /// A mutable reference to the current execution context.
    fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }
}

#[cfg(all(test, feature = "bulk-load-profile"))]
mod tests {
    use super::{Connection, DirectPacketWritePart, DirectPacketWriteTiming, MaybeTlsStream};
    use crate::tds::{codec::PacketCodec, Context, HEADER_BYTES};
    use asynchronous_codec::Framed;
    use bytes::BytesMut;
    use futures_util::io::{AsyncRead, AsyncWrite};
    use std::{
        collections::VecDeque,
        io,
        pin::Pin,
        task::{self, Poll},
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum WriteAction {
        Pending,
        Write(usize),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FlushAction {
        Pending,
        Ready,
    }

    #[derive(Debug, Default)]
    struct ScriptedStream {
        writes: VecDeque<WriteAction>,
        flushes: VecDeque<FlushAction>,
        written: Vec<u8>,
    }

    impl ScriptedStream {
        fn with_writes(writes: impl IntoIterator<Item = WriteAction>) -> Self {
            Self {
                writes: writes.into_iter().collect(),
                flushes: VecDeque::new(),
                written: Vec::new(),
            }
        }

        fn with_flushes(flushes: impl IntoIterator<Item = FlushAction>) -> Self {
            Self {
                writes: VecDeque::new(),
                flushes: flushes.into_iter().collect(),
                written: Vec::new(),
            }
        }
    }

    impl AsyncRead for ScriptedStream {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut task::Context<'_>,
            _buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Ok(0))
        }
    }

    impl AsyncWrite for ScriptedStream {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            match self.writes.pop_front() {
                Some(WriteAction::Pending) => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Some(WriteAction::Write(bytes)) => {
                    let accepted = bytes.min(buf.len());
                    self.written.extend_from_slice(&buf[..accepted]);
                    Poll::Ready(Ok(accepted))
                }
                None => {
                    self.written.extend_from_slice(buf);
                    Poll::Ready(Ok(buf.len()))
                }
            }
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> Poll<io::Result<()>> {
            match self.flushes.pop_front() {
                Some(FlushAction::Pending) => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Some(FlushAction::Ready) | None => Poll::Ready(Ok(())),
            }
        }

        fn poll_close(self: Pin<&mut Self>, _cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[async_std::test]
    async fn direct_packet_write_helper_records_full_header_write() {
        let mut stream = MaybeTlsStream::Raw(ScriptedStream::with_writes([WriteAction::Write(8)]));
        let mut timing = DirectPacketWriteTiming::default();

        Connection::<ScriptedStream>::write_all_direct_packet_bytes(
            &mut stream,
            b"abcdefgh",
            DirectPacketWritePart::Header,
            &mut timing,
        )
        .await
        .expect("header write should succeed");

        let MaybeTlsStream::Raw(stream) = stream else {
            unreachable!();
        };
        assert_eq!(stream.written, b"abcdefgh");
        assert_eq!(timing.write_calls, 1);
        assert_eq!(timing.write_bytes, 8);
        assert_eq!(timing.max_write_bytes, 8);
        assert_eq!(timing.header_write_calls, 1);
        assert_eq!(timing.header_write_bytes, 8);
        assert_eq!(timing.header_max_write_bytes, 8);
        assert_eq!(timing.header_partial_writes, 0);
        assert_eq!(timing.payload_write_calls, 0);
        assert_eq!(timing.poll_write_polls, 1);
        assert_eq!(timing.poll_write_pending_count, 0);
        assert_eq!(timing.poll_write_ready_count, 1);
    }

    #[async_std::test]
    async fn direct_packet_write_records_stream_mode_and_packet_parts() {
        let mut connection = Connection {
            transport: Framed::new(
                MaybeTlsStream::Raw(ScriptedStream::with_writes([WriteAction::Write(
                    HEADER_BYTES + 5,
                )])),
                PacketCodec,
            ),
            context: Context::new(),
            flushed: true,
            buf: BytesMut::new(),
        };

        let mut packet = BytesMut::from(&[0_u8; HEADER_BYTES][..]);
        packet.extend_from_slice(b"abcde");
        let timing = connection
            .write_direct_packet_buffer_with_timing(&packet)
            .await
            .expect("direct packet buffer write should succeed");

        assert!(!connection.flushed);
        assert!(timing.raw_stream);
        assert!(!timing.tls_stream);
        assert_eq!(timing.header_bytes, HEADER_BYTES);
        assert_eq!(timing.payload_bytes, 5);
        assert_eq!(timing.header_write_calls, 1);
        assert_eq!(
            timing.header_write_bytes,
            u64::try_from(HEADER_BYTES).unwrap()
        );
        assert_eq!(timing.payload_write_calls, 1);
        assert_eq!(timing.payload_write_bytes, 5);
        assert_eq!(timing.write_calls, 1);
        assert_eq!(timing.write_bytes, u64::try_from(HEADER_BYTES + 5).unwrap());
        assert_eq!(timing.flush_pending_count, 0);
    }

    #[async_std::test]
    async fn direct_packet_contiguous_write_handles_partial_header_and_payload_writes() {
        let mut stream = MaybeTlsStream::Raw(ScriptedStream::with_writes([
            WriteAction::Write(2),
            WriteAction::Write(4),
            WriteAction::Write(7),
        ]));
        let mut timing = DirectPacketWriteTiming::default();

        Connection::<ScriptedStream>::write_all_direct_packet_contiguous(
            &mut stream,
            b"abcdefgh12345",
            &mut timing,
        )
        .await
        .expect("contiguous direct packet write should succeed");

        let MaybeTlsStream::Raw(stream) = stream else {
            unreachable!();
        };
        assert_eq!(stream.written, b"abcdefgh12345");
        assert_eq!(timing.write_calls, 3);
        assert_eq!(timing.write_bytes, 13);
        assert_eq!(timing.max_write_bytes, 7);
        assert_eq!(timing.header_write_calls, 3);
        assert_eq!(timing.header_write_bytes, 8);
        assert_eq!(timing.header_partial_writes, 2);
        assert_eq!(timing.payload_write_calls, 1);
        assert_eq!(timing.payload_write_bytes, 5);
        assert_eq!(timing.payload_partial_writes, 0);
    }

    #[async_std::test]
    async fn direct_packet_write_helper_records_pending_and_partial_payload_writes() {
        let mut stream = MaybeTlsStream::Raw(ScriptedStream::with_writes([
            WriteAction::Pending,
            WriteAction::Write(2),
            WriteAction::Write(3),
        ]));
        let mut timing = DirectPacketWriteTiming::default();

        Connection::<ScriptedStream>::write_all_direct_packet_bytes(
            &mut stream,
            b"abcde",
            DirectPacketWritePart::Payload,
            &mut timing,
        )
        .await
        .expect("payload write should succeed");

        let MaybeTlsStream::Raw(stream) = stream else {
            unreachable!();
        };
        assert_eq!(stream.written, b"abcde");
        assert_eq!(timing.write_calls, 2);
        assert_eq!(timing.write_bytes, 5);
        assert_eq!(timing.max_write_bytes, 3);
        assert_eq!(timing.payload_write_calls, 2);
        assert_eq!(timing.payload_write_bytes, 5);
        assert_eq!(timing.payload_max_write_bytes, 3);
        assert_eq!(timing.payload_partial_writes, 1);
        assert_eq!(timing.header_write_calls, 0);
        assert_eq!(timing.poll_write_polls, 3);
        assert_eq!(timing.poll_write_pending_count, 1);
        assert_eq!(timing.poll_write_ready_count, 2);
    }

    #[async_std::test]
    async fn direct_packet_write_helper_rejects_zero_byte_write() {
        let mut stream = MaybeTlsStream::Raw(ScriptedStream::with_writes([WriteAction::Write(0)]));
        let mut timing = DirectPacketWriteTiming::default();

        let err = Connection::<ScriptedStream>::write_all_direct_packet_bytes(
            &mut stream,
            b"abc",
            DirectPacketWritePart::Payload,
            &mut timing,
        )
        .await
        .expect_err("zero-byte write should fail");

        assert!(matches!(err, crate::Error::Io { .. }));
        assert_eq!(timing.write_calls, 0);
        assert_eq!(timing.write_bytes, 0);
        assert_eq!(timing.poll_write_polls, 1);
        assert_eq!(timing.poll_write_ready_count, 1);
    }

    #[async_std::test]
    async fn direct_packet_flush_helper_records_pending_flush() {
        let mut stream = MaybeTlsStream::Raw(ScriptedStream::with_flushes([
            FlushAction::Pending,
            FlushAction::Ready,
        ]));
        let mut timing = DirectPacketWriteTiming::default();

        Connection::<ScriptedStream>::poll_direct_packet_flush(&mut stream, &mut timing)
            .await
            .expect("flush should succeed");

        assert_eq!(timing.flush_pending_count, 1);
    }
}
