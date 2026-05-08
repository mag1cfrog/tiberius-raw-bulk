# Fork Notes

`tiberius-raw-bulk` is a minimal fork of
[`prisma/tiberius`](https://github.com/prisma/tiberius).

## Upstream Base

- Upstream repository: `https://github.com/prisma/tiberius`
- Upstream package: `tiberius`
- Upstream base version: `0.12.3`
- Upstream base commit: `a6b4fcdae0de5702427290b89f8d05bc51f3bcfa`

## Fork Purpose

This fork exists to expose narrow raw bulk-load extension points that are not
available from upstream Tiberius today. Those extension points are intended for
callers that need to construct SQL Server bulk-copy row payloads directly while
still using Tiberius for connection management, metadata discovery, packet
framing, flushing, and finalization.

The fork should remain close to upstream Tiberius. General TDS behavior,
connection behavior, token handling, query behavior, and runtime support should
continue to follow upstream unless a raw bulk-load extension requires a focused
change.

Arrow-specific planning, encoding, policy, and validation logic does not belong
in this repository. That logic belongs in `arrow-tiberius`; this fork should
stay useful as a Tiberius-compatible client package with minimal raw bulk-load
support.

## Package Shape

The crates.io package is named `tiberius-raw-bulk`, but the Rust library crate
is intentionally still named `tiberius`:

```toml
tiberius = { package = "tiberius-raw-bulk", version = "0.12.3-raw-bulk.1" }
```

That lets downstream code continue importing `tiberius::Client` while selecting
this fork explicitly at the Cargo package level.
