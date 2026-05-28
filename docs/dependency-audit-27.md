# Dependency Audit for Issue 27

Date: 2026-05-28

Issue: https://github.com/mag1cfrog/tiberius-raw-bulk/issues/27

Branch: `chore/27-dependency-audit`

This document records the dependency audit progress for `tiberius-raw-bulk`.
It is intentionally version-controlled so progress survives context compaction,
review handoff, and later phase work in `arrow-tiberius`.

## Progress

1. Dependency inventory: complete for the initial baseline.
2. Maintenance risk: complete for the initial baseline.
3. Feature minimization and default feature health: complete for the initial baseline.
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

## Step 2: Maintenance Risk

### Commands Used

```sh
cargo audit
cargo audit --json
cargo deny check advisories bans sources
cargo outdated --workspace --root-deps-only
cargo tree --workspace --target all --duplicates
cargo tree --workspace --target all --invert <crate>
cargo tree -p tiberius-raw-bulk --target all --no-default-features --features <feature> --invert <crate>
cargo info <crate>@<version>
```

Crates.io metadata was also checked for notable root and transitive crates on
2026-05-28.

### Advisory Summary

`cargo audit` scanned 375 lockfile dependencies against 1098 RustSec
advisories. It reported:

- 3 vulnerabilities
- 9 advisory warnings

Important classification: `cargo audit` scans the full lockfile. A finding can
come from the default runtime graph, an optional feature graph, a target-specific
graph, or dev/test dependencies.

### Vulnerabilities

| Advisory | Crate | Version | Classification | Path | Initial action |
| --- | --- | --- | --- | --- | --- |
| `RUSTSEC-2026-0098` | `rustls-webpki` | `0.101.7` | Optional `rustls` feature and dev-only `reqwest` path | `tokio-rustls 0.24.1 -> rustls 0.21.12 -> rustls-webpki`; also `reqwest 0.11.27` in dev deps | Upgrade the rustls stack or remove stale dev paths. |
| `RUSTSEC-2026-0099` | `rustls-webpki` | `0.101.7` | Optional `rustls` feature and dev-only `reqwest` path | Same as above | Same as above. |
| `RUSTSEC-2026-0104` | `rustls-webpki` | `0.101.7` | Optional `rustls` feature and dev-only `reqwest` path | Same as above | Same as above. |

The default feature set uses `native-tls`, not `rustls`. These vulnerabilities
are not on the default TLS backend path, but they are real for users enabling
the optional `rustls` feature and for dev/test builds that pull `reqwest`.

The current optional rustls dependency path is:

```text
rustls
-> tokio-rustls 0.24.1
   -> rustls 0.21.12
      -> rustls-webpki 0.101.7
-> rustls-native-certs 0.6.3
   -> rustls-pemfile 1.0.4
-> rustls-pemfile 1.0.4
```

`cargo outdated` reports newer roots:

- `tokio-rustls 0.26.4`
- `rustls-native-certs 0.8.3`
- `rustls-pemfile 2.2.0`

This is likely a larger compatibility migration, not only a lockfile update.
`cargo info tokio-rustls@0.26.4` also shows that the old
`dangerous_configuration` feature is obsolete in the newer line.

### Advisory Warnings

| Advisory | Crate | Version | Kind | Classification | Path | Initial action |
| --- | --- | --- | --- | --- | --- | --- |
| `RUSTSEC-2025-0052` | `async-std` | `1.13.2` | unmaintained | Optional `sql-browser-async-std` feature and dev/test direct dependency | Direct optional dependency and direct dev dependency | Decide whether to keep, deprecate, or document the async-std SQL Browser feature. |
| `RUSTSEC-2024-0375` | `atty` | `0.2.14` | unmaintained | Dev/test only | `env_logger 0.9.3`, `names 0.14.0 -> clap 3.2.25` | Upgrade `env_logger`; consider replacing `names`. |
| `RUSTSEC-2021-0145` | `atty` | `0.2.14` | unsound | Dev/test only, Windows-specific advisory | Same as above | Same as above. |
| `RUSTSEC-2024-0384` | `instant` | `0.1.13` | unmaintained | Optional `sql-browser-smol` feature and dev-only Azure path | `async-io 1.13.0`, `async-net 1.8.0`, `futures-lite 1.13.0`; also `azure_identity` dev path | Test bumping smol stack roots to `async-io 2`, `async-net 2`, `futures-lite 2`. |
| `RUSTSEC-2025-0119` | `number_prefix` | `0.4.0` | unmaintained | Dev/test only | `indicatif 0.17.11` | Check whether `indicatif 0.18.4` removes or changes this path. |
| `RUSTSEC-2024-0436` | `paste` | `1.0.15` | unmaintained | Dev/test direct dependency and dev Azure path | Direct dev dependency; also `azure_core` via `azure_identity` | Review direct test usage in step 4; dev Azure update may still pull it. |
| `RUSTSEC-2024-0370` | `proc-macro-error` | `1.0.4` | unmaintained | Dev/test only | `names 0.14.0 -> clap 3.2.25 -> clap_derive` | Consider replacing `names`; this also reduces `syn 1` duplication. |
| `RUSTSEC-2025-0134` | `rustls-pemfile` | `1.0.4` | unmaintained | Optional `rustls` feature and dev-only `reqwest` path | Direct optional `rustls-pemfile`; `rustls-native-certs 0.6.3`; `reqwest 0.11.27` dev path | Upgrade rustls stack and dev HTTP auth stack. |
| `RUSTSEC-2026-0097` | `rand` | `0.7.3` | unsound | Default Windows auth path and dev Azure path | `winauth 0.0.4`; `azure_identity -> http-types` | `winauth 0.0.5` still depends on `rand 0.7`; a simple bump does not remove this warning. |

### Classification by Build Surface

| Surface | Findings | Notes |
| --- | --- | --- |
| Core no-default runtime | No RustSec findings found in this pass | The compact 33-entry core graph is clean under the current lockfile audit. |
| Default native TLS | No direct RustSec finding in this pass | The default `native-tls` path is heavy but not the source of current advisories. |
| Default Windows auth | `rand 0.7.3` unsound warning | This only resolves on Windows targets but is part of default features. |
| Optional `rustls` | `rustls-webpki` vulnerabilities; `rustls-pemfile` unmaintained | Highest-priority optional-feature risk because it is security-sensitive TLS code. |
| Optional `sql-browser-async-std` | `async-std` unmaintained | The feature is explicitly tied to a discontinued runtime. |
| Optional `sql-browser-smol` | `instant` unmaintained through old smol stack crates | Likely fixable by moving to the current smol stack. |
| Dev/test | `atty`, `number_prefix`, `paste`, `proc-macro-error`, dev paths to `rustls-webpki`, `rustls-pemfile`, and `rand` | Large maintenance surface mostly from AAD auth test support, logging, random names, and test macros. |

### Outdated Root Dependencies

`cargo outdated --workspace --root-deps-only` reported these root dependency
updates:

| Package | Dependency | Current | Latest | Kind | Classification |
| --- | --- | ---: | ---: | --- | --- |
| `runtimes-macro` | `darling` | `0.14.4` | `0.23.0` | normal | Dev/test macro maintenance. |
| `runtimes-macro` | `syn` | `1.0.109` | `2.0.117` | normal | Dev/test macro maintenance; may remove `syn 1` duplication. |
| `tiberius-raw-bulk` | `async-io` | `1.13.0` | `2.6.0` | normal optional | `sql-browser-smol`; likely addresses old smol stack. |
| `tiberius-raw-bulk` | `async-native-tls` | `0.4.0` | `0.6.0` | normal optional/default TLS | Default TLS dependency. Newer `runtime-async-std` feature is obsolete. |
| `tiberius-raw-bulk` | `async-net` | `1.8.0` | `2.0.0` | normal optional | `sql-browser-smol`; likely addresses old smol stack. |
| `tiberius-raw-bulk` | `asynchronous-codec` | `0.6.2` | `0.7.0` | normal | Core runtime. |
| `tiberius-raw-bulk` | `azure_identity` | `0.5.0` | `1.0.0` | dev | AAD auth test/example support. |
| `tiberius-raw-bulk` | `bigdecimal` | `0.3.1` | `0.4.10` | normal optional | Optional data type feature. |
| `tiberius-raw-bulk` | `env_logger` | `0.9.3` | `0.11.10` | dev | Likely removes `atty`. |
| `tiberius-raw-bulk` | `futures-lite` | `1.13.0` | `2.6.1` | normal optional | `sql-browser-smol`; likely addresses old smol stack. |
| `tiberius-raw-bulk` | `indicatif` | `0.17.11` | `0.18.4` | dev | May affect `number_prefix` path. |
| `tiberius-raw-bulk` | `indoc` | `1.0.9` | `2.0.7` | dev | Test utility. |
| `tiberius-raw-bulk` | `libgssapi` | `0.8.3` | `0.9.1` | normal target `cfg(unix)` | Optional GSSAPI. |
| `tiberius-raw-bulk` | `oauth2` | `4.4.2` | `5.0.0` | dev | AAD auth test/example support. |
| `tiberius-raw-bulk` | `pretty-hex` | `0.3.0` | `0.4.2` | normal | Core runtime utility. |
| `tiberius-raw-bulk` | `reqwest` | `0.11.27` | `0.13.4` | dev | Dev HTTP stack; contributes rustls advisories. |
| `tiberius-raw-bulk` | `rustls-native-certs` | `0.6.3` | `0.8.3` | normal optional | Optional rustls stack. |
| `tiberius-raw-bulk` | `rustls-pemfile` | `1.0.4` | `2.2.0` | normal optional | Optional rustls stack; advisory warning. |
| `tiberius-raw-bulk` | `thiserror` | `1.0.69` | `2.0.18` | normal | Core runtime. |
| `tiberius-raw-bulk` | `tokio` | `1.52.2` | `1.52.3` | normal optional/dev | Runtime/test support. |
| `tiberius-raw-bulk` | `tokio-rustls` | `0.24.1` | `0.26.4` | normal optional | Optional rustls stack; fixes vulnerable webpki path. |
| `tiberius-raw-bulk` | `winauth` | `0.0.4` | `0.0.5` | normal target `cfg(windows)` | Default Windows auth; latest still uses `rand 0.7`. |

`cargo outdated` also warned that:

- `async-native-tls` latest no longer has `runtime-async-std`.
- `tokio-rustls` latest no longer has `dangerous_configuration`.

Those are migration notes for step 3, not simple version-number edits.

### Maintenance Metadata Snapshot

Selected crates.io metadata checked on 2026-05-28:

| Crate | Latest | Last crates.io update | Repository | Risk note |
| --- | ---: | --- | --- | --- |
| `async-native-tls` | `0.6.0` | `2026-02-20` | `async-email/async-native-tls` | Active enough; feature migration needed. |
| `tokio-rustls` | `0.26.4` | `2025-09-26` | `rustls/tokio-rustls` | Active; upgrade likely needed for rustls advisory fix. |
| `rustls-native-certs` | `0.8.3` | `2025-12-29` | `rustls/rustls-native-certs` | Active; part of rustls migration. |
| `rustls-pemfile` | `2.2.0` | `2024-09-30` | `rustls/pemfile` | Has unmaintained advisory for old line; API may migrate toward `rustls-pki-types`. |
| `winauth` | `0.0.5` | `2024-03-22` | `steffengy/winauth-rs` | Low activity; latest still uses `rand 0.7`. |
| `connection-string` | `0.2.0` | `2023-03-31` | `prisma/connection-string` | No advisory, but stale and core runtime. |
| `pretty-hex` | `0.4.2` | `2026-03-15` | `wolandr/pretty-hex` | Active enough; root is outdated. |
| `asynchronous-codec` | `0.7.0` | `2023-10-11` | `mxinden/asynchronous-codec` | Somewhat stale; root is outdated. |
| `async-std` | `1.13.2` | `2025-08-15` | `async-rs/async-std` | RustSec discontinued warning. |
| `async-io` | `2.6.0` | `2025-09-14` | `smol-rs/async-io` | Active enough; current root uses old major. |
| `async-net` | `2.0.0` | `2023-10-29` | `smol-rs/async-net` | Current root uses old major. |
| `futures-lite` | `2.6.1` | `2025-08-04` | `smol-rs/futures-lite` | Active enough; current root uses old major. |
| `azure_identity` | `1.0.0` | `2026-05-12` | `azure/azure-sdk-for-rust` | Active; dev root is very old. |
| `reqwest` | `0.13.4` | `2026-05-25` | `seanmonstar/reqwest` | Active; dev root is old and contributes rustls advisory path. |
| `oauth2` | `5.0.0` | `2025-01-21` | `ramosbugs/oauth2-rs` | Active enough; dev root is old. |
| `env_logger` | `0.11.10` | `2026-03-23` | `rust-cli/env_logger` | Active; dev root is old. |
| `names` | `0.14.0` | `2022-06-28` | `fnichol/names` | Stale dev root; pulls old clap/proc-macro-error path. |
| `indicatif` | `0.18.4` | `2026-02-14` | `console-rs/indicatif` | Active; dev root is outdated. |
| `darling` | `0.23.0` | `2025-12-03` | `TedDriggs/darling` | Active; macro crate uses old `0.14`. |
| `syn` | `2.0.117` | `2026-02-20` | `dtolnay/syn` | Active; macro crate uses old `1`. |

### Step 2 Findings

- Highest priority security issue: optional `rustls` currently resolves to
  vulnerable `rustls-webpki 0.101.7`. This affects users enabling `rustls` and
  dev/test builds through `reqwest`.
- Highest priority default-feature issue: default Windows auth resolves to
  `rand 0.7.3` through `winauth`. `winauth 0.0.5` does not remove that risk.
- Default native TLS is heavy but did not trigger the current RustSec findings.
- The async-std SQL Browser feature has a structural maintenance problem:
  `async-std` is discontinued.
- The smol SQL Browser feature is on old `async-io`, `async-net`, and
  `futures-lite` roots that pull unmaintained `instant`; this looks more
  fixable than the async-std issue.
- The dev/test dependency surface is the largest source of stale and
  unmaintained crates. A focused dev-dependency refresh could remove several
  advisories without changing runtime behavior.

### Step 2 Follow-ups for Step 3

1. Treat the rustls stack as a focused migration branch.
2. Treat Windows auth as a design decision: either accept/document the current
   `winauth` risk, fork/patch it, or replace the auth implementation.
3. Test low-blast-radius root bumps separately: `pretty-hex`, `tokio`,
   `env_logger`, `indoc`, and possibly `indicatif`.
4. Test smol SQL Browser upgrades as a separate feature branch.
5. Decide whether to keep, deprecate, or document `sql-browser-async-std`.
6. Review direct dev usage of `paste`, `names`, `azure_identity`, `oauth2`, and
   `reqwest` before upgrading or replacing them.

## Step 3: Feature Minimization and Default Feature Health

### Commands Used

```sh
cargo check -p tiberius-raw-bulk --no-default-features
cargo check -p tiberius-raw-bulk
cargo check -p tiberius-raw-bulk --all-features
cargo check -p tiberius-raw-bulk --tests --no-default-features
cargo check -p tiberius-raw-bulk --tests
cargo check -p tiberius-raw-bulk --no-default-features --features native-tls
cargo check -p tiberius-raw-bulk --no-default-features --features rustls
cargo check -p tiberius-raw-bulk --no-default-features --features opentls
cargo check -p tiberius-raw-bulk --no-default-features --features vendored-openssl
cargo check -p tiberius-raw-bulk --no-default-features --features sql-browser-tokio
cargo check -p tiberius-raw-bulk --no-default-features --features sql-browser-smol
cargo check -p tiberius-raw-bulk --no-default-features --features sql-browser-async-std
cargo check -p tiberius-raw-bulk --no-default-features --features integrated-auth-gssapi
cargo check -p tiberius-raw-bulk --no-default-features --features 'chrono time rust_decimal bigdecimal bulk-load-profile tds73'
rustup target list --installed
cargo info windows
cargo info winauth@0.0.5
cargo info sspi
cargo info mssql-auth
cargo info ntlmclient
cargo info kenobi
```

Candidate graph counts were also measured in a temporary manifest under
`/tmp/tiberius-dep-audit-step3`. Those scratch files are not part of this repo.

### Compile Matrix

| Feature selection | Result | Notes |
| --- | --- | --- |
| `--no-default-features` | Pass | Two existing dead-code warnings because TLS code is not enabled. |
| default features | Pass | Default is `tds73`, `winauth`, and `native-tls`; on Linux the Windows-only `winauth` dependency is not built. |
| `--tests --no-default-features` | Pass | Same dead-code warnings as the no-default library check. |
| `--tests` | Pass | Default test build compiles on this host. |
| `native-tls` | Pass | Current default TLS backend compiles. |
| `rustls` | Pass | Compiles, but remains blocked by the RustSec findings from step 2. |
| `opentls` | Pass | Direct OpenTLS feature compiles. |
| `vendored-openssl` | Pass | Feature aliases through `opentls` and compiles. |
| `sql-browser-tokio` | Pass | Compiles. |
| `sql-browser-smol` | Pass | Compiles, but still uses old smol-stack roots from step 2. |
| `sql-browser-async-std` | Pass | Compiles, but depends on discontinued `async-std`. |
| data/profile group | Pass | Checked `chrono`, `time`, `rust_decimal`, `bigdecimal`, `bulk-load-profile`, and `tds73` together. |
| `integrated-auth-gssapi` | Blocked by host setup | `libgssapi-sys` found runtime libraries but failed because `gssapi.h` is not installed. |
| `--all-features` | Blocked by same host setup | Fails for the same `gssapi.h` reason, not from a Rust type-check error in this crate. |

Only `x86_64-unknown-linux-gnu` is installed locally, so Windows `winauth`
compile coverage was not verified in this pass.

### Default Feature Policy

Current defaults:

```text
default = ["tds73", "winauth", "native-tls"]
```

Recommendation for this fork: keep the high-level default capabilities for now.

- Keep `tds73` by default. It has no dependency impact and is expected protocol
  functionality.
- Keep TLS by default. SQL Server clients normally need encrypted connections,
  and disabling TLS by default would be a worse user-facing behavior than
  carrying a larger default graph.
- Keep Windows auth as a default capability, but do not keep the current
  `winauth` implementation indefinitely. The goal should be "Windows auth stays
  default" while the dependency behind it changes.

This means step 3 is not recommending a minimal default in the strict Cargo
sense. It is recommending a compatibility-focused default with targeted
remediation of the risky default dependency.

### TLS Decision Notes

`native-tls` is the right default TLS backend for now:

- It compiles on the current host.
- It did not trigger the current RustSec findings in step 2.
- It is heavy, but the weight mostly comes from expected platform TLS,
  OpenSSL, URL, IDNA, and ICU paths.

The optional `rustls` backend should stay optional, but it needs a focused
migration:

- Current `tokio-rustls 0.24.1` resolves to vulnerable
  `rustls-webpki 0.101.7`.
- Newer `tokio-rustls` no longer has the old `dangerous_configuration` feature,
  so this is not a simple version bump.
- This should be handled as a separate compatibility branch with tests for
  certificate validation and trust-root behavior.

`opentls` and `vendored-openssl` compile, but `opentls` is not part of the
default feature set. It should be reviewed in step 4 for actual usage and
necessity before investing in it.

### Windows Auth Replacement Assessment

The current Windows auth dependency is `winauth 0.0.4`. The latest checked
release, `winauth 0.0.5`, still depends on `rand 0.7`, so a direct bump does
not remove `RUSTSEC-2026-0097`.

Replacement is preferable to patching or forking `winauth`, provided the
replacement supports both public auth modes:

- `AuthMethod::Integrated`: current Windows logon session.
- `AuthMethod::Windows`: explicit `DOMAIN\user` or user/password credentials.

Current code only needs a narrow token interface:

```text
initial_token = provider.next_token(None)
response_token = provider.next_token(Some(server_sspi_token))
```

That makes an internal adapter trait feasible and keeps the TDS login flow
mostly unchanged.

Candidate snapshot:

| Candidate | Latest checked | MSRV | Approx normal graph count | Assessment |
| --- | ---: | ---: | ---: | --- |
| `windows` direct SSPI adapter | `0.62.2` | `1.82` | 20 | Best spike candidate. Maintained by Microsoft, much smaller than `sspi`, and matches this crate's Windows-only scope. Need to verify explicit credential support with `AcquireCredentialsHandleW` auth data. |
| `sspi` | `0.21.0` | `1.89` | 273 | Active and feature-rich, but raises MSRV and has a very large graph. Also needs verification for current-logon integrated auth behavior. |
| `mssql-auth` | `0.10.0` | `1.88` | 264 | SQL Server-specific and its negotiator shape matches our handshake. However `sspi-auth` pulls `sspi`; useful as a reference or possible upstream collaboration target, less attractive as a direct dependency. |
| `ntlmclient` | `0.2.0` | unknown | 82 | Explicit NTLM client only. Does not clearly cover current-user integrated auth. |
| `kenobi` | `0.4.1` | unknown | 26 | Small graph, but cross-platform Negotiate pulls GSSAPI on Unix and needs deeper compatibility proof for SQL Server SSPI tokens. |
| patched or forked `winauth` | n/a | unknown | similar to current | Last resort only. It keeps us responsible for auth crypto and maintenance. |

Preferred path:

1. Keep the public feature name and public auth API stable for now.
2. Add an internal `WindowsSspiProvider` adapter behind the existing `winauth`
   feature, or introduce a replacement feature with compatibility aliases if
   the name is changed later.
3. Spike a native Windows SSPI implementation using the maintained `windows`
   crate first.
4. Verify both `Integrated` and explicit `Windows` auth against SQL Server on
   Windows before removing `winauth`.
5. Use `mssql-auth` and `sspi` as references if native SSPI becomes too large
   or fails explicit credentials.
6. Only patch or fork `winauth` if the maintained replacement path fails.

### SQL Browser Runtime Decision Notes

`sql-browser-tokio` should remain the preferred maintained SQL Browser runtime.
It compiles and is on the ecosystem's dominant async runtime.

`sql-browser-smol` compiles, but the root dependency versions are stale. This
should be a separate low-risk bump attempt to `async-io 2`, `async-net 2`, and
`futures-lite 2`.

`sql-browser-async-std` compiles, but depends on a discontinued runtime. The
least surprising policy is to keep it temporarily for compatibility, document
the maintenance warning, and decide in a later issue whether to deprecate it.

### Step 3 Findings

- No-default and default builds compile on this host.
- The current default feature set is reasonable as a user-facing policy, but
  its Windows auth implementation needs replacement.
- Replacing `winauth` is better than patching it if a native SSPI adapter can
  cover both current-user and explicit credential auth.
- A direct `windows` crate adapter looks like the best first spike because it
  has a much smaller graph and lower MSRV than `sspi` or `mssql-auth`.
- `rustls` should remain optional until its security-sensitive migration is
  complete.
- `integrated-auth-gssapi` needs CI or local setup with GSSAPI development
  headers before it can be included in all-feature validation.

### Step 3 Follow-ups for Step 4

1. Review actual public API and test usage of each optional feature.
2. Check whether `opentls` and `vendored-openssl` are still necessary.
3. Identify direct usage of `paste`, `names`, AAD dev dependencies, and
   progress/logging dev dependencies before changing them.
4. Draft the Windows SSPI replacement spike as a separate implementation item.
