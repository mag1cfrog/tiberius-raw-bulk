# Dependency Audit for Issue 27

Date: 2026-05-28

Issue: https://github.com/mag1cfrog/tiberius-raw-bulk/issues/27

Branch: `chore/27-dependency-audit`

This document records the dependency audit progress for `tiberius-raw-bulk`.
It is intentionally version-controlled so progress survives context compaction,
review handoff, and later phase work in `arrow-tiberius`.

## Progress

1. Dependency inventory: complete for the initial baseline.
2. Maintenance risk: pending.
3. Feature minimization and default feature health: pending.
4. API surface and dependency necessity: pending.
5. Supply-chain policy: pending.
6. Build and graph impact: pending.

## Step 1: Dependency Inventory

### Workspace Shape

The workspace has two packages:

| Package | Kind | Notes |
| --- | --- | --- |
| `tiberius-raw-bulk` | library | Main crate, published as library name `tiberius`. |
| `runtimes-macro` | proc macro | Workspace member used as a dev dependency by tests. |

The repository currently has:

- `Cargo.toml`
- `Cargo.lock`
- `runtimes-macro/Cargo.toml`

No `deny.toml` or GitHub workflow files were present during this inventory.

### Commands Used

```sh
cargo metadata --format-version 1 --no-deps
cargo tree -p tiberius-raw-bulk --target all --edges normal --no-default-features
cargo tree -p tiberius-raw-bulk --target all --edges normal
cargo tree -p tiberius-raw-bulk --target all --edges features
cargo tree --workspace --edges normal,build,dev --duplicates
```

Additional feature-specific graph counts used:

```sh
cargo tree -p tiberius-raw-bulk --target all --edges normal --prefix none --no-default-features
cargo tree -p tiberius-raw-bulk --target all --edges normal --prefix none
cargo tree -p tiberius-raw-bulk --target all --edges normal --prefix none --no-default-features --features <feature>
```

### Core Runtime Dependencies

These are non-optional, non-dev direct dependencies of the main crate.

| Dependency | Requirement | Default features | Explicit features | Inventory note |
| --- | --- | --- | --- | --- |
| `async-trait` | `0.1` | yes | none | Proc macro for async trait support. |
| `asynchronous-codec` | `0.6` | yes | none | Framed codec support. |
| `byteorder` | `1.0` | yes | none | Binary protocol encoding/decoding. |
| `bytes` | `1.0` | yes | none | Buffer types. |
| `connection-string` | `0.2` | yes | none | Connection string parsing. |
| `encoding_rs` | `0.8` | yes | none | Text encoding support. |
| `enumflags2` | `0.7` | yes | none | Bitflag enums. |
| `futures-util` | `0.3` | no | `io`, `sink` | Async IO and sink traits. |
| `num-traits` | `0.2` | yes | none | Numeric conversions. |
| `once_cell` | `1.3` | yes | none | Lazy/static initialization. |
| `pin-project-lite` | `0.2` | yes | none | Pin projection helper. |
| `pretty-hex` | `0.3` | yes | none | Hex debug formatting. |
| `thiserror` | `1.0` | yes | none | Error derive. |
| `tracing` | `0.1` | yes | `log` | Runtime diagnostics. |
| `uuid` | `1.0` | yes | none | UUID type support. |

No-default runtime graph size with `--target all`: 33 unique package/version
entries, including the root package.

### Optional Runtime Dependencies

These are optional direct dependencies controlled by feature flags.

| Dependency | Requirement | Default features | Explicit features | Feature path |
| --- | --- | --- | --- | --- |
| `async-native-tls` | `0.4` | yes | `runtime-async-std` | `native-tls -> async-native-tls` |
| `async-io` | `1.8` | yes | none | `sql-browser-smol -> async-io` |
| `async-net` | `1.7` | yes | none | `sql-browser-smol -> async-net` |
| `async-std` | `1` | yes | `attributes` | `sql-browser-async-std -> async-std` |
| `bigdecimal` as `bigdecimal_` | `0.3` | yes | none | `bigdecimal -> bigdecimal_` |
| `chrono` | `0.4` | no | none | `chrono` |
| `futures-lite` | `1.12.0` | yes | none | `sql-browser-smol -> futures-lite` |
| `opentls` | `0.2.1` | yes | `io-async-std`, `vendored` | `opentls` and `vendored-openssl -> opentls` |
| `rust_decimal` | `1.6` | yes | none | `rust_decimal` |
| `rustls-native-certs` | `0.6` | yes | none | `rustls -> rustls-native-certs` |
| `rustls-pemfile` | `1` | yes | none | `rustls -> rustls-pemfile` |
| `time` | `0.3` | yes | none | `time` |
| `tokio` | `1.0` | yes | `net`, `time` | `sql-browser-tokio -> tokio` |
| `tokio-rustls` | `0.24.0` | yes | `dangerous_configuration` | `rustls -> tokio-rustls` |
| `tokio-util` | `0.7` | yes | `compat` | `sql-browser-tokio` and `rustls` |

### Target-Specific Optional Dependencies

| Dependency | Target | Requirement | Default features | Feature path |
| --- | --- | --- | --- | --- |
| `winauth` | `cfg(windows)` | `0.0.4` | yes | `winauth -> dep:winauth` |
| `libgssapi` | `cfg(unix)` | `0.8.1` | no | `integrated-auth-gssapi -> libgssapi` |

`winauth` is part of the default feature set, but it only resolves on Windows
targets. The `--target all` graph is required to see it from a non-Windows
audit machine.

### Dev Dependencies

The main crate has a large dev/test dependency surface:

| Dependency | Requirement | Explicit features | Inventory note |
| --- | --- | --- | --- |
| `anyhow` | `1` | none | Test error handling. |
| `async-std` | `1` | `attributes` | Test runtime. |
| `azure_identity` | `0.5.0` | none | AAD auth example/test support. |
| `chrono` | `0.4.38` | none | Test date/time support. |
| `env_logger` | `0.9` | none | Test logging. |
| `indicatif` | `0.17` | none | Test/example progress output. |
| `indoc` | `1.0.7` | none | Test SQL strings. |
| `names` | `0.14` | none | Random test names. |
| `oauth2` | `4.2.3` | none | AAD auth support. |
| `paste` | `1.0` | none | Test macros. |
| `reqwest` | `0.11.10` | none | AAD auth HTTP support. |
| `runtimes-macro` | path dependency | none | Runtime matrix test macro. |
| `tokio` | `1.0` | `macros`, `sync`, `io-std`, `time`, `io-util`, `net`, `rt-multi-thread` | Test runtime. |
| `tokio-util` | `0.7` | `compat` | Test compatibility helpers. |
| `url` | `2.2.2` | none | Test URL support. |
| `uuid` | `1.0` | `v4` | Test UUID generation. |

Initial duplicate-version output shows many duplicates are dev-only or mostly
dev-driven, especially through `azure_identity`, `reqwest`, `oauth2`, `names`,
`env_logger`, and `runtimes-macro`. Step 2 will classify the security and
maintenance impact by runtime vs dev-only.

### `runtimes-macro` Dependencies

| Dependency | Requirement | Default features |
| --- | --- | --- |
| `darling` | `0.14` | yes |
| `proc-macro2` | `1` | yes |
| `quote` | `1` | yes |
| `syn` | `1` | yes |

This proc macro currently pulls `syn 1`, while the main runtime graph also
uses `syn 2` through modern proc macro dependencies. This is dev-only unless
the macro crate is built directly.

### Feature Map

| Feature | Direct dependency effect |
| --- | --- |
| `default` | `tds73`, `winauth`, `native-tls` |
| `all` | `chrono`, `time`, `tds73`, SQL Browser runtimes, GSSAPI, decimal features, `native-tls`, `bulk-load-profile` |
| `tds73` | no direct dependency |
| `winauth` | `dep:winauth` on Windows |
| `native-tls` | `async-native-tls` |
| `async-native-tls` | `dep:async-native-tls` |
| `rustls` | `tokio-rustls`, `tokio-util`, `rustls-pemfile`, `rustls-native-certs` |
| `opentls` | `dep:opentls` |
| `vendored-openssl` | `opentls` |
| `sql-browser-tokio` | `tokio`, `tokio-util` |
| `sql-browser-async-std` | `async-std` |
| `sql-browser-smol` | `async-io`, `async-net`, `futures-lite` |
| `integrated-auth-gssapi` | `libgssapi` on Unix |
| `chrono` | `dep:chrono` |
| `time` | `dep:time` |
| `rust_decimal` | `dep:rust_decimal` |
| `bigdecimal` | `bigdecimal_` |
| `bulk-load-profile` | no direct dependency |
| `docs` | no direct dependency |

### Runtime Graph Counts

Counts are unique package/version entries from `cargo tree --target all
--edges normal --prefix none`, including the root package.

| Feature selection | Count | Inventory note |
| --- | ---: | --- |
| No default features | 33 | Core protocol/runtime graph. |
| Default features | 104 | Core plus `tds73`, `winauth`, and `native-tls`. |
| `native-tls` only | 89 | Largest default contributor. Includes platform TLS, OpenSSL, URL, IDNA, and ICU paths. |
| `winauth` only | 49 | Windows-only auth path. Includes `rand 0.7`, old `getrandom`, `md5`, and `winapi`. |
| `rustls` only | 66 | Optional rustls backend path. |
| `opentls` only | 70 | Optional OpenTLS path with vendored settings on the dependency. |
| `sql-browser-tokio` only | 41 | Tokio SQL Browser support. |
| `sql-browser-async-std` only | 75 | Async-std SQL Browser support. |
| `sql-browser-smol` only | 76 | Smol SQL Browser support. |
| `chrono` only | 34 | One additional direct optional dependency. |
| `time` only | 38 | Time crate support. |
| `rust_decimal` only | 38 | Decimal type support. |
| `bigdecimal` only | 36 | BigDecimal type support. |
| `all` | 182 | Everything enabled, target all. |

### Notable Runtime Paths

Default feature set:

```text
default = ["tds73", "winauth", "native-tls"]
```

`tds73` has no direct dependency impact.

`native-tls` path:

```text
native-tls
-> async-native-tls
   -> native-tls
      -> openssl / schannel / security-framework / tempfile
   -> url
      -> idna
         -> ICU crates
```

`winauth` path:

```text
winauth
-> winauth 0.0.4
   -> bitflags 1.3.2
   -> byteorder
   -> md5 0.6.1
   -> rand 0.7.3
      -> getrandom 0.1.16
      -> rand_core 0.5.1
   -> winapi 0.3.9
```

SQL Browser feature paths:

```text
sql-browser-tokio -> tokio, tokio-util
sql-browser-async-std -> async-std
sql-browser-smol -> async-io, async-net, futures-lite
```

Data type feature paths:

```text
chrono -> chrono
time -> time
rust_decimal -> rust_decimal
bigdecimal -> bigdecimal_
```

### Step 1 Findings

- The default dependency graph is intentionally featureful because TLS and
  Windows auth are default capabilities.
- `native-tls` is the largest default feature contributor by package count.
- `winauth` is target-specific but important to audit with `--target all`.
- Dev dependencies are much heavier than the core runtime graph and should be
  classified separately in security and maintenance findings.
- The no-default core runtime graph is relatively small and gives a useful
  baseline for measuring feature-specific impact.

### Next Step

Step 2 should use this inventory to classify advisories, unmaintained crates,
outdated root dependencies, and duplicate versions as:

- core runtime
- default TLS
- default Windows auth
- optional non-default feature
- dev/test only
