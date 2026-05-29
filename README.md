# tiberius-raw-bulk
[![crates.io](https://img.shields.io/crates/v/tiberius-raw-bulk.svg)](https://crates.io/crates/tiberius-raw-bulk)
[![docs.rs](https://docs.rs/tiberius-raw-bulk/badge.svg)](https://docs.rs/tiberius-raw-bulk)

Focused raw bulk-load fork of
[`prisma/tiberius`](https://github.com/prisma/tiberius), a native Microsoft SQL
Server (TDS) client for Rust.

The package is published as `tiberius-raw-bulk`, but the Rust library crate name
remains `tiberius`:

```toml
tiberius = { package = "tiberius-raw-bulk", version = "0.12.3-raw-bulk.12" }
```

This fork keeps upstream behavior intact outside the raw bulk-load extension
points. Application-specific planning, Arrow mapping, and row encoding logic
belong in downstream crates.

See [FORK.md](FORK.md) for upstream base and maintenance notes.

## Raw Bulk Extensions

This fork adds stable extension points for callers that want to plan or encode
bulk rows outside Tiberius while still using Tiberius for connection handling
and TDS packet framing.

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

Profiling and statistics APIs are opt-in through `bulk-load-profile`.

## Feature Flags

| Flag                     | Description                                                                                                                      | Default    |
|--------------------------|----------------------------------------------------------------------------------------------------------------------------------|------------|
| `tds73`                  | Support for new date and time types in TDS version 7.3. Disable if using version 7.2.                                            | `enabled`  |
| `native-tls`             | Use operating system's TLS libraries for traffic encryption.                                                                     | `enabled`  |
| `rustls`                 | Use the builtin TLS implementation from rustls instead of linking to the operating system implementation for traffic encryption. | `disabled` |
| `openssl`                | Use OpenSSL for traffic encryption, dynamically linking to a system OpenSSL installation.                                       | `disabled` |
| `vendored-openssl`       | Statically link against OpenSSL instead of dynamically linking to the operating system implementation for traffic encryption.    | `disabled` |
| `chrono`                 | Read and write date and time values using `chrono`'s types. (for greenfield, using time instead of chrono is recommended)        | `disabled` |
| `time`                   | Read and write date and time values using `time` crate types.                                                                    | `disabled` |
| `rust_decimal`           | Read and write `numeric`/`decimal` values using `rust_decimal`'s `Decimal`.                                                      | `disabled` |
| `bigdecimal`             | Read and write `numeric`/`decimal` values using `bigdecimal`'s `BigDecimal`.                                                     | `disabled` |
| `bulk-load-profile`      | Expose raw bulk-load profiling and statistics APIs, including packet counters and write timing breakdowns.                       | `disabled` |
| `sql-browser-async-std`  | SQL Browser implementation for the `TcpStream` of async-std.                                                                     | `disabled` |
| `sql-browser-tokio`      | SQL Browser implementation for the `TcpStream` of Tokio.                                                                         | `disabled` |
| `sql-browser-smol`       | SQL Browser implementation for the `TcpStream` of smol.                                                                          | `disabled` |
| `integrated-auth-gssapi` | Support for using Integrated Auth via GSSAPI                                                                                     | `disabled` |

## Compatibility Notes

Tiberius accepts any socket that implements `AsyncRead` and `AsyncWrite` from
the `futures` crate. TCP streams from async-std, Tokio, and Smol can be used
with the matching compatibility adapters.

TLS is enabled by default through `native-tls`. Use `rustls`, `openssl`, or
`vendored-openssl` if those match your deployment better. The crate can also be
compiled without TLS support, but not with multiple TLS backends enabled at the
same time.

With `integrated-auth-gssapi` enabled on Unix-like platforms, the build requires
GSSAPI/Kerberos libraries and headers.

`bulk-load-profile` is disabled by default. Starting with
`0.12.3-raw-bulk.12`, bulk-load packet stats, write timing stats, combined
stats, and stats-returning finalization methods are only available when this
feature is enabled.

## Maintenance

This fork is maintained for downstream raw bulk-load work. Upstream protocol,
query, authentication, and general client behavior should continue to come from
Prisma Tiberius unless a raw bulk-load extension requires a focused change.

For upstream Tiberius issues, report upstream. For fork-specific raw bulk-load
behavior, report an issue in this fork.
