# proxy-protocol

This implements the [PROXY protocol] created by [HAProxy] in [Rust].

## Licence

This is licensed under either:

  * Apache Licence, version 2.0 ([LICENCE-APACHE-2.0] or <https://www.apache.org/licenses/LICENSE-2.0>)
  * MIT Licence ([LICENCE-MIT] or <https://choosealicense.com/licenses/mit/>)

at your discretion.

### Contributing

All contributions must be submitted under both licences and cannot have any
additional terms or conditions.

[PROXY protocol]: ./proxy-protocol.txt
[HAProxy]: https://www.haproxy.org/
[Rust]: https://www.rust-lang.org/
[LICENCE-APACHE-2.0]: ./LICENCE-APACHE-2.0
[Apache-2.0]: https://www.apache.org/licenses/LICENSE-2.0
[LICENCE-MIT]: ./LICENCE-MIT

## Release Process

`proxy-protocol` is published to the private `gen0sec` Cargo registry
(`https://crates-internal.g0s.dev`). Releases are driven by `vX.Y.Z` git
tags — `.github/workflows/release.yaml` handles build, publish, and
GitHub Release creation.

### One-time setup

- **Repo secret**: `GEN0SEC_CARGO_TOKEN` — bearer token for the registry.
- **`release` GitHub environment** (Settings → Environments → New) for
  optional manual approval gating before publish.
- **Local cargo config** (`~/.cargo/config.toml`) for developers:

  ```toml
  [registries.gen0sec]
  index = "sparse+https://crates-internal.g0s.dev/api/v1/crates/"
  credential-provider = ["cargo:token"]
  token = "<your_token>"
  ```

### Cutting a release

```bash
cargo install cargo-release
cargo release patch --execute   # or minor / major / 1.2.3
```

`cargo-release` bumps `Cargo.toml` + `Cargo.lock`, commits, creates
`vX.Y.Z`, and pushes; CI publishes on the tag push.
`[package.metadata.release] publish = false` keeps the local command
from publishing — that is left to CI.

### CI jobs on `v*` tag

1. **`verify-version`** — fails if tag does not match `Cargo.toml`.
2. **`package-crate`** — `cargo package --registry gen0sec --locked`,
   uploads `proxy-protocol-X.Y.Z.crate`.
3. **`publish`** — `release` environment-gated `cargo publish` with
   retry/timeout hardening.
4. **`gh-release`** — downloads `.crate`, generates SHA256 sidecars,
   creates a GitHub Release.

### Required Cargo.toml metadata

The gen0sec registry enforces these `[package]` fields, missing ones
cause a `400 Bad Request`:

`name`, `description`, `repository`, `license`, `authors`, `categories`,
`keywords`, `links`, `readme`.
