# tiberius-raw-bulk

[![crates.io](https://img.shields.io/crates/v/tiberius-raw-bulk.svg)](https://crates.io/crates/tiberius-raw-bulk)
[![docs.rs](https://docs.rs/tiberius-raw-bulk/badge.svg)](https://docs.rs/tiberius-raw-bulk)

`tiberius-raw-bulk` is a focused fork of
[`prisma/tiberius`](https://github.com/prisma/tiberius), a native Microsoft SQL
Server TDS client for Rust.

This fork keeps upstream Tiberius behavior intact outside its raw bulk-load
extension points. It is intended for callers that already know how to plan and
encode bulk rows, but still want Tiberius to manage SQL Server connections, TLS,
login, TDS packet framing, and server responses.

Application-specific planning, Arrow mapping, schema mapping, and row encoding
belong in downstream crates.

See [FORK.md](FORK.md) for the upstream base and maintenance policy.

## Install

The package is published as `tiberius-raw-bulk`, but the Rust library crate name
remains `tiberius` for upstream compatibility:

```toml
[dependencies]
tiberius = { package = "tiberius-raw-bulk", version = "0.12.3-raw-bulk.15" }
```

Default features enable TDS 7.3, Windows authentication support, and
`native-tls`.

## What This Fork Adds

This fork adds raw bulk-load extension points and sanitized protocol
observability.

Use it when you need to:

- inspect destination table metadata before starting a bulk-load request,
- build a bulk-load request from previously discovered column metadata,
- append already encoded TDS row payloads,
- write bulk packets directly to the connection buffer,
- collect optional bulk-load packet and write timing statistics,
- observe protocol lifecycle events without exposing SQL text, row values,
  credentials, packet bytes, or server message text.

Use upstream Tiberius directly when you only need regular query execution,
normal typed bulk insertion, or general SQL Server connectivity without the raw
bulk-load API surface.

## Connecting

Tiberius is runtime-independent. Create the TCP stream in your runtime, adapt it
to the `futures` `AsyncRead` and `AsyncWrite` traits when needed, then pass it
to `Client::connect`.

Tokio example:

```rust
use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = Config::new();
    config.host("localhost");
    config.port(1433);
    config.authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"));
    config.trust_cert();

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let mut client = Client::connect(config, tcp.compat_write()).await?;
    client.query("SELECT 1", &[]).await?;

    Ok(())
}
```

`async-std`, `smol`, and other runtimes can be used with compatible streams and
adapters.

## Raw Bulk Load

Raw bulk-load APIs are for callers that already own row encoding. The crate does
not inspect or validate row values in raw payloads beyond API-specific boundary
checks.

Metadata discovery and request setup:

- `Client::bulk_insert_columns()`
- `Client::bulk_insert_with_columns()`
- `BulkLoadRequest::columns()`

Raw row append APIs:

- `BulkLoadRequest::send_raw_row_payload()`
- `BulkLoadRequest::send_raw_rows_payload()`
- `BulkLoadRequest::send_raw_rows_payload_checked()`
- `BulkLoadRequest::send_raw_rows_with()`
- `RawRowsAppendBuffer`

Direct packet writes:

- `BulkLoadRequest::enable_direct_packet_writes()`
- `BulkLoadRequest::direct_packet_writes_enabled()`

`bulk-load-profile` adds public profiling and statistics APIs, including packet
counters, write timing breakdowns, and stats-returning finalization methods.
The feature is disabled by default.

## Observability

This crate emits `tracing` spans and events for stable, sanitized client and
protocol telemetry. It never installs a subscriber. Applications, tests, or
downstream crates own subscriber installation and exporter wiring.

The intended layering is:

- the application or orchestrator installs the subscriber,
- application request spans wrap higher-level work,
- downstream crates such as `arrow-tiberius` may add writer lifecycle spans,
- `tiberius-raw-bulk` emits client and protocol spans/events under those spans.

Exporter integration belongs to the application. This crate does not depend on
or configure OpenTelemetry, Datadog, Grafana, Loki, metrics, or logging
exporters.

Stable protocol telemetry uses:

- target: `tiberius_raw_bulk::protocol`
- event field: `telemetry_event`
- span and event names: dotted names, for example
  `protocol.bulk_load.request`
- structured field names: snake_case
- count fields: `_count` suffix
- byte fields: `_bytes` suffix
- elapsed duration fields: `_elapsed_ms` suffix

Stable spans currently emitted by this crate:

| Span | Scope |
|------|-------|
| `protocol.connection.connect` | TDS connection setup after the caller supplies an open stream |
| `protocol.tls.negotiation` | TLS negotiation |
| `protocol.login.flow` | Login and authentication flow |
| `protocol.bulk_load.request` | Bulk-load request lifecycle |

Stable event groups currently emitted by this crate:

- Connection and prelogin:
  - `protocol.connection.setup.start`
  - `protocol.connection.setup.completed`
  - `protocol.connection.setup.failed`
  - `protocol.connection.prelogin.start`
  - `protocol.connection.prelogin.completed`
  - `protocol.connection.prelogin.failed`
- TLS and login:
  - `protocol.tls.negotiation.start`
  - `protocol.tls.negotiation.completed`
  - `protocol.tls.negotiation.failed`
  - `protocol.tls.trust_config`
  - `protocol.tls.root_certificates.loaded`
  - `protocol.tls.post_login.downgraded`
  - `protocol.login.flow.start`
  - `protocol.login.flow.completed`
  - `protocol.login.flow.failed`
- Bulk load:
  - `protocol.bulk_load.request.start`
  - `protocol.bulk_load.request.completed`
  - `protocol.bulk_load.request.failed`
  - `protocol.bulk_load.packet.written`
  - `protocol.bulk_load.flush.completed`
  - `protocol.bulk_load.flush.failed`
- Server tokens:
  - `protocol.token.col_metadata`
  - `protocol.token.row`
  - `protocol.token.nbc_row`
  - `protocol.token.return_value`
  - `protocol.token.return_status`
  - `protocol.token.order`
  - `protocol.token.done`
  - `protocol.token.error`
  - `protocol.token.info`
  - `protocol.token.env_change`
  - `protocol.token.login_ack`
  - `protocol.token.feature_ext_ack`
  - `protocol.token.sspi`
- SQL Browser:
  - `protocol.sql_browser.resolution.start`
  - `protocol.sql_browser.resolution.completed`
  - `protocol.sql_browser.resolution.timeout`
  - `protocol.sql_browser.resolution.failed`

Fields intended to be stable enough for downstream assertions include:

- `telemetry_event`, `phase`, `operation`, `status`
- safe protocol categories such as `requested_encryption`,
  `negotiated_encryption`, `server_encryption`, `tls_backend`, `auth_method`,
  `trust_mode`, `token_kind`, `env_change_kind`, `runtime`,
  `address_family`, and `error_category`
- safe numeric summaries such as packet sizes, packet counts, row counts when
  known, SQL Browser ports, TLS root certificate counts, DONE status booleans,
  server error/info codes, state, class, and line numbers

Default stable tracing must not emit:

- connection strings, usernames, passwords, access tokens, auth payload bytes,
  or credential material,
- raw SQL text, parameter values, row values, return value payloads, or raw
  token debug output,
- raw packet bytes, SQL Browser request bytes, or SQL Browser response bytes,
- certificate DER bytes or certificate PEM contents,
- arbitrary server-returned ERROR or INFO message text,
- ENVCHANGE string values such as database names, usernames, server names,
  mirror names, routing host names, or full socket addresses,
- SQL Browser instance names, host names, or full socket addresses.

ERROR and INFO token telemetry reports safe structured metadata only, such as
number or code, state, class, line number, token kind, and phase. DONE telemetry
reports safe status booleans and row count only when SQL Server marks the DONE
row count as valid. Row, NBC row, return value, and SSPI telemetry reports only
safe token metadata.

Baseline tracing is available without `bulk-load-profile`. The
`bulk-load-profile` feature adds public profiling/statistics APIs; it is not
required to receive protocol tracing events.

Formatted event messages and internal debug output are not part of the stable
contract. Downstream tests and collectors should assert on structured fields,
especially `target`, `telemetry_event`, span names, and the stable field names
listed above.

## Feature Flags

| Flag | Description | Default |
|------|-------------|---------|
| `tds73` | Support TDS 7.3 date and time types. Disable for TDS 7.2 compatibility. | enabled |
| `native-tls` | Use the operating system TLS libraries for traffic encryption. | enabled |
| `rustls` | Use rustls for traffic encryption. | disabled |
| `openssl` | Use OpenSSL, dynamically linked to the system OpenSSL installation. | disabled |
| `vendored-openssl` | Use OpenSSL with a vendored static build. | disabled |
| `winauth` | Enable Windows authentication support. | enabled |
| `chrono` | Read and write date and time values using `chrono` types. | disabled |
| `time` | Read and write date and time values using `time` crate types. | disabled |
| `rust_decimal` | Read and write `numeric`/`decimal` values using `rust_decimal::Decimal`. | disabled |
| `bigdecimal` | Read and write `numeric`/`decimal` values using `bigdecimal::BigDecimal`. | disabled |
| `bulk-load-profile` | Expose raw bulk-load profiling and statistics APIs. | disabled |
| `sql-browser-async-std` | SQL Browser implementation for `async-std::net::TcpStream`. Deprecated for new code. | disabled |
| `sql-browser-tokio` | SQL Browser implementation for Tokio `TcpStream`. | disabled |
| `sql-browser-smol` | SQL Browser implementation for `async_net::TcpStream`. | disabled |
| `integrated-auth-gssapi` | Enable Integrated Auth through GSSAPI on Unix-like platforms. | disabled |
| `aad-auth-example` | Build the Azure Active Directory authentication example dependencies. | disabled |
| `docs` | Documentation build cfg; not needed by application builds. | disabled |

Only one TLS backend can be enabled at a time. The TLS backend can also be
disabled entirely, but encrypted SQL Server connections require a TLS backend.

## Compatibility Notes

Tiberius accepts any stream that implements `AsyncRead` and `AsyncWrite` from
the `futures` crate. TCP streams from Tokio, Smol, and async-std can be used
with the matching compatibility adapters.

`sql-browser-async-std` is deprecated because `async-std` is discontinued
upstream. The feature remains available for existing users who pass
`async_std::net::TcpStream` to Tiberius. New code should prefer
`sql-browser-smol` with `async_net::TcpStream` as the closest migration path, or
`sql-browser-tokio` when the application already uses Tokio.

SQL Browser tracing is emitted only when one of `sql-browser-async-std`,
`sql-browser-tokio`, or `sql-browser-smol` is enabled and named-instance
resolution is used. Its `runtime` field is one of `async_std`, `tokio`, or
`smol`.

TLS is enabled by default through `native-tls`. Use `rustls`, `openssl`, or
`vendored-openssl` if those match your deployment better. The observability
`tls_backend` field uses safe values such as `native_tls`, `rustls`,
`vendored_openssl`, or `none`.

With `integrated-auth-gssapi` enabled on Unix-like platforms, the build requires
GSSAPI/Kerberos libraries and headers.

## Maintenance

This fork is maintained for downstream raw bulk-load work. Upstream protocol,
query, authentication, and general client behavior should continue to come from
Prisma Tiberius unless a raw bulk-load extension requires a focused change.

For upstream Tiberius issues, report upstream. For fork-specific raw bulk-load
behavior, report an issue in this fork.
