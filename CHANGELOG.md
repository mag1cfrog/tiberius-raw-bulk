# Changes

## Version 0.12.3-raw-bulk.13

- Complete the dependency audit follow-up work.
- Add `cargo-deny` CI coverage for advisories, bans, duplicates, and sources.
- Replace `winauth` with an internal Windows SSPI adapter.
- Keep the default Windows authentication feature available.
- Update smol SQL Browser dependency roots.
- Remove the old `instant` dependency path from smol SQL Browser support.
- Document `sql-browser-async-std` as deprecated.
- Keep `sql-browser-async-std` available for compatibility.
- Update `runtimes-macro` to current `darling` and `syn 2` roots.
- Harden CI with updated actions and Cargo network retry settings.
- Improve CI caching and use official SQL Server images.
- Avoid repeated Windows SQL Server setup in CI.
- Keep macOS CI focused on client-side compile and library tests.

## Version 0.12.3-raw-bulk.12

- Add the `bulk-load-profile` feature for raw bulk-load profiling and
  statistics APIs.
- Remove bulk-load packet stats, write timing stats, combined stats, and
  stats-returning finalization methods from the default build. Consumers that
  need those APIs must enable `bulk-load-profile`.
- Keep normal bulk writes and direct packet writes available without
  profiling enabled.

## Version 0.12.3-raw-bulk.11

- Coalesce each experimental direct bulk packet into one contiguous
  header-plus-payload write buffer.
- Keep the direct-mode pending buffer header-prefixed so packet headers can be
  patched in place while bulk payload packet splitting stays inside
  `BulkLoadRequest`.
- Preserve direct packet profiling counters for header and payload byte
  attribution across contiguous and partial low-level writes.
- Add SQL Server bulk-load coverage for direct packet writes over raw and TLS
  streams with payloads large enough to require packet splitting.

## Version 0.12.3-raw-bulk.10

- Add deeper experimental direct packet write profiling:
  - header vs payload write calls, bytes, elapsed time, maxima, and partial
    write counters;
  - `poll_write` poll, pending, and ready counters with elapsed-time
    breakdowns;
  - direct packet flush pending counters and elapsed-time breakdowns;
  - raw stream vs TLS stream packet counters;
  - final `EndOfMessage` direct packet counters.
- Keep direct packet writes opt-in through
  `BulkLoadRequest::enable_direct_packet_writes()`.
- Keep normal query writes and default bulk writes on the existing framed packet
  sink.

## Version 0.12.3-raw-bulk.9

- Add an opt-in raw bulk direct packet writer for benchmark comparison:
  - `BulkLoadRequest::enable_direct_packet_writes()`;
  - `BulkLoadRequest::direct_packet_writes_enabled()`;
  - `BulkLoadDirectPacketWriteStats`;
  - `BulkLoadWriteTimingStats::direct_packet_write`.
- Keep the default raw bulk path and all normal query writes on the existing
  framed packet sink.
- Preserve TDS packet framing in the direct path by serializing the same packet
  header type, status, id, and payload bytes without introducing unsafe code.

## Version 0.12.3-raw-bulk.8

- Add deeper raw bulk connection-write timing statistics:
  - `BulkLoadConnectionWriteStats`;
  - `BulkLoadWriteTimingStats::connection_write`.
- Split raw bulk framed sink writes into readiness, packet encode/start-send,
  and sink flush timing while keeping normal non-bulk writes on the existing
  `SinkExt::send` path.
- Preserve existing combined stats access through `BulkLoadRequest::stats()`
  and `BulkLoadRequest::finalize_with_stats()`.

## Version 0.12.3-raw-bulk.7

- Add request-scoped bulk-load write timing statistics:
  - `BulkLoadWriteTimingStats`;
  - `BulkLoadStats`;
  - `BulkLoadRequest::write_timing_stats()`;
  - `BulkLoadRequest::stats()`;
  - `BulkLoadRequest::finalize_with_stats()`.
- Preserve `BulkLoadRequest::finalize_with_packet_stats()` compatibility.
- Track bulk-load packet-drain timing, lower-level write timing and payload
  bytes, flush timing, max write/flush timings, and finalization timing
  breakdowns for downstream benchmarks.

## Version 0.12.3-raw-bulk.6

- Add configurable TDS login packet size support:
  - `Config::packet_size()`;
  - ADO.NET and JDBC connection-string keys `Packet Size`, `PacketSize`, and
    `packet_size`.
- Keep the default requested packet size unchanged at `4096`.
- Continue using SQL Server's negotiated packet size for runtime packet
  splitting.

## Version 0.12.3-raw-bulk.5

- Add low-overhead bulk-load packet write statistics:
  - `BulkLoadPacketStats`;
  - `BulkLoadRequest::packet_stats()`;
  - `BulkLoadRequest::finalize_with_packet_stats()`.
- Keep `BulkLoadRequest::finalize()` compatible by discarding packet stats.
- Track packet drain calls, packet payload bytes, buffered tails, and final
  packet payload size for downstream benchmark profiling.

## Version 0.12.3-raw-bulk.4

- Add rollback-safe direct raw bulk buffer append API:
  - `BulkLoadRequest::send_raw_rows_with()`;
  - `RawRowsAppend`;
  - `RawRowsAppendBuffer`.
- Keep raw bulk packet splitting and flushing inside `BulkLoadRequest`.
- Validate appended row-token offsets relative to the newly appended buffer
  region.
- Roll back appended bytes if caller encoding or row-token validation fails.

## Version 0.12.3-raw-bulk.3

- Fix bulk-load metadata for TDS 7.3 temporal types:
  - encode `date` type metadata without an invalid type-length byte;
  - render `time(n)` with its fractional seconds scale in `INSERT BULK`;
  - render `datetimeoffset(n)` with its fractional seconds scale in `INSERT BULK`.
- Add SQL Server bulk-load regression tests for `date`, `time(3)`,
  `datetime2(3)`, and `datetimeoffset(3)` values.
- Add side-effect-free bulk target metadata discovery:
  - `Client::bulk_insert_columns()`;
  - `Client::bulk_insert_with_columns()`;
  - `BulkLoadColumns`.
- Keep `Client::bulk_insert()` compatible by delegating through the split
  metadata discovery and bulk-start flow.

## Version 0.12.3-raw-bulk.2

- Relax `Client::bulk_insert` table-name borrowing so callers can pass a
  locally formatted table SQL string without extending it to the returned
  `BulkLoadRequest` lifetime.

## Version 0.12.3-raw-bulk.1

- Rename the Cargo package to `tiberius-raw-bulk`.
- Keep the Rust library crate name as `tiberius` for downstream compatibility.
- Add fork attribution and maintenance notes.
- Expose read-only bulk-load destination metadata through `BulkLoadRequest::columns()`.
- Add raw bulk row send APIs:
  - `BulkLoadRequest::send_raw_row_payload()`;
  - `BulkLoadRequest::send_raw_rows_payload()`;
  - `BulkLoadRequest::send_raw_rows_payload_checked()`.
- Keep upstream Tiberius 0.12.3 behavior unchanged outside the raw bulk-load extension points.

## Version 0.12.3
- feat: improve column type accuracy (#347)
- fix: encoding of zero-length values for large varlen columns (#315)
- update tokio_rustls (#306)
- Allow iterating over the cells in a row. (#303)
- Send ReadOnlyIntent when ApplicationIntent=ReadOnly specified (#297)
- Replace encoding with encoding_rs (#285)
- Disable chrono's oldtime feature (#284)

## Version 0.12.2

- Update connection-string crate to 0.2 (#286)

## Version 0.12.1

- fix: bigdecimal conversion overflow (#271)
- Reduce futures crate dependency footprint (#270)

## Version 0.12.0

- BREAKING: Correctly convert DateTimeOffset to/from database (#269)
  Please read the [issue](https://github.com/prisma/tiberius/issues/260)
  carefully before upgrading.

## Version 0.11.8

- feat: improve column type info (#347)

## Version 0.11.7

- chore: Update connection string to 0.2 (#286)

## Version 0.11.6

- fix: bigdecimal conversion overflow (#271)

## Version 0.11.5

- Close connection explicitly (#268)

## Version 0.11.4

- Fix buffer overrun on finalize (#266)
- Correctly parse (local) server name (#259)

## Version 0.11.3

- Cleanup TokenRow public API (#255)
- Fix null values in NBC rows (#253)

## Version 0.11.2

- Fix error ordering (#248)

## Version 0.11.1

- Don't load native roots for trust-all config (#243)
- Propagate errors correctly (#247)

## Version 0.11.0

- BREAKING: bigdecimal crate upgraded to 0.3 major and has to be of
  the same major in other crates using Tiberius.
- Handle negative scale from a BigDecimal (#240)

## Version 0.10.0

- BREAKING: uuid crate upgraded to 1.0 major and has to be of the same
  major in other crates using Tiberius.

## Version 0.9.5

- Add fractional seconds precision for datetime2 (#235)

## Version 0.9.4

- Fix SQL Browser response parsing error (#229)
- Bulk uploads (#227)

## Version 0.9.3

- Enable SSL if using vendored-openssl feature (#225)

## Version 0.9.2

- Allow statically linking against OpenSSL (#222)

## Version 0.9.1

- Support AAD token authentication (#215)

## Version 0.9.0

- (BREAKING) support rustls, switch between native-tls and rustls.
  the feature flag vendored-openssl is gone. instead if needing vendored TLS,
  use feature flag rustls

## Version 0.8.0

- (BREAKING) fix: correctly decode null integers (#209)

## Version 0.7.3

- Fixing an accidentally renamed time module, that would've been a breaking change.

## Version 0.7.2

- Dynamic query interface (#196)
- Support for `time` 0.3.x (#201)
- Additional option to add custom-ca to root certificates (#203, thx @lostiniceland)

## Version 0.7.1

- Support all pre-login tokens

## Version 0.7.0

- Remove async-std from deps if using tokio
- show TokioAsyncWriteCompatExt in Client docs (#183)
- Upgrade to Rust edition 2021 (#180)

## Version 0.6.5

- Constrain UUID features and optionalize winauth dependency (smaller binaries)

## Version 0.6.4

- Use bundled bigint from bigdecimal

## Version 0.6.3

- Bignum/bigint compilation problems fixed.

## Version 0.6.2

- Improvement on waker calls. We used to wake the runtime too often, this should improve performance.

## Version 0.6.1

- SQL Browser for the smol runtime.

## Version 0.6.0

- Refactor stream handling to something more rusty (#166). This is a breaking
  change, if relying on the asynchronous stream handling of QueryResult. Please
  refer to the updated documentation.

## Version 0.5.16

- Allow setting application name per connection (#161)

## Version 0.5.15

- Split column decoding into modules (speeding up TEXT/NTEXT/IMAGE decoding a lot) (#153)

## Version 0.5.14

- Handle collations for CHAR and TEXT values (#153)

## Version 0.5.13

- Add Config parsing for "Integrated Security" (two words)
- Unified bitflag setup
- Correct default ports
- Update to enumflags2 0.7

## Version 0.5.12

- Warnings should not affect metadata fetching (#139)

## Version 0.5.11

- Fixing of all clippy warnings. This might have some performance benefits and
might also fix some weird bugs in environments where we can't guarantee the
evaluation order. (#136)
- Add info of LCID and sort id to colation errors (#138)

## Version 0.5.10

- Remove a rogue `dbg!`

## Version 0.5.9

- Set the `app_name` in LOGIN7 to `tiberius`. This allows connecting to servers
  that expect the value to not be empty (see issue #127).

## Version 0.5.8

- Try out all resolved IP addresses (#124)

## Version 0.5.7

- Set server name in the login packet (#122)

## Version 0.5.6

-  Fix for handling nullable values (#119 #121)

## Version 0.5.5 and 0.4.21

Catastropichal build failures with feature flags fixed.

## Version 0.5.4 and 0.4.20

Removed the tls feature flag to simplify dependencies. This means you will
always get a TLS-enabled build, and can disable it on runtime. This also means
we don't always compile async-std if wanting to use tokio, and so forth.

Fixes certain issues with vendored OpenSSL on macOS platforms too.

## Version 0.5.3

Changed futures-codec2 to asynchronous-codec, due to former was yanked.

## Versions 0.5.2 and 0.4.19

Introducing working TLS support on macOS platforms.

Please read the issue:

https://github.com/prisma/tiberius/issues/65

## Version 0.5.1

Internally upgrade bytes to 1.0. Should have no visible change to the apis.

## Version 0.5.0

If using Tiberius with Tokio and SQL Browser, this PR will upgrade Tokio to 1.0.

0.4 branch will be updated for a short while if needed and until the ecosystem
has completely settled on Tokio 1.0.

## Version 0.4.18

- Allow `databaseName` in connection string to define the database (#108)
- Implement reader functions for standard string data (#107)
- Fix a time conversion error (#106)

## Version 0.4.17

- Fixing error swallowing with `simple_query` and MARS (#105)
- Fixing transaction descriptor reading (#105)
- Fixing envchange token reads (#105)

## Version 0.4.16

- Handle all MARS results properly (#102)

## Version 0.4.14

- Support alternatively `BigNumber` when dealing with numeric values.
- Document feature flags

## Version 0.4.13

- Realizing UTF-16 works just fine with SQL Server. Reverting the UCS2, but
  still keeping the faster writes.

## Version 0.4.12

*SKIP this, go directly to 0.4.13*

- A typo fix in README (#94)
- Faster string writes with better length handling. UCS2 for writes (#95).

## Version 0.4.11

- Allow disabling TLS in connection string (#89)
- Use connection-string for ado.net parsing (#91)
- Handle JDBC connection strings (#92)

## Version 0.4.10

- Handling nullable int values, fix for #78 (#80)
- Reflect tweaks to upstream libgssapi crate (#81)
- Skip default features in libgssapi (for macOS support)
- Handle env change Routing request (#87)

## Version 0.4.9

- BREAKING: `AuthMethod::WindowsIntegrated` renamed to `AuthMethod::Integrated`.
- Use GSSAPI for IntegratedSecurity on Unix platforms
- Fix module docs for examples
- Make `packet_id` wrapping explicit
- Add DNS feature to Tokio

## Version 0.4.8

- BREAKING: `ColumnData::I8(i8)` is now `ColumnData::U8(u8)` due to misunderstanding how `tinyint` works. (#71)
- Skip any received `done_rows` amounts and avoid creating extra resultsets (#67)
- Actually run the chrono tests (#72)
- Fix GUID byte ordering (#69)
- Fix null time/datetime2/datetimeoffset handling (#73)
- Null image data should be `Binary`, not `String`

## Version 0.4.7

- Pass hostname to TLS handshake, allowing usage with AzureSQL using
  `TrustServerCertificate=no`
  ([#62](https://github.com/prisma/tiberius/pull/62))

## Version 0.4.5

- Documenting type conversions and re-exporting chrono types
  ([#60](https://github.com/prisma/tiberius/pull/60))

## Version 0.4.4

- Fixing multi-part table names in `IMAGE`, `TEXT` and `NTEXT` column metadata
  ([#58](https://github.com/prisma/tiberius/pull/58))

## Version 0.4.3

- Starting transactions with `simple_query` now works without crashing
  ([#55](https://github.com/prisma/tiberius/pull/55))

## Version 0.4.2

- Fixing old and wrong `ExecuteResult` docs
- Adding `rows_affected` method to `ExecuteResult`

## Version 0.4.1

- Add all feature flags for docs.rs build

## Version 0.4.0

- A complete rewrite from 0.3.0
- Not bound to Tokio anymore, independent of the runtime
- Support for many more types
- Async/await, futures 0.3
