# Release and compatibility policy

## Support baseline

- Rust edition: 2024.
- Minimum supported Rust version (MSRV): 1.89. CI checks the workspace with Rust 1.89 and builds release binaries with stable Rust.
- Active platform target: Linux x86_64 GNU. Windows and macOS remain planned ports; their code is not currently in the release validation target.
- Runtime validation is currently Linux-only. A successful build does not certify daemon IPC permissions or game launch behavior on another operating system.
- UMC 0.1 supports vanilla Minecraft Java Edition installation and explicit offline launch. Microsoft authentication and loader/modpack support are outside this release scope.

## Version compatibility

Rust crate versions and the JSON-RPC protocol version evolve independently.

- Rust crates use SemVer. Before 1.0, a breaking Rust API change requires a new minor version; patch releases remain compatible. After 1.0, breaking public API changes require a major version.
- JSON-RPC `v1` is advertised via `system.info`. Add methods and optional fields compatibly; clients should ignore unknown fields and event names. Do not change the meaning or remove a `v1` method, field, or event. A breaking wire change requires a separately advertised protocol version.
- `system.capabilities` is the feature discovery mechanism. Clients must not infer capabilities from the Core crate version.
- The RPC error mapping and some response fields remain provisional during 0.x and must be documented before a 1.0 compatibility promise.
- The instance configuration carries its own `schema_version`; every new on-disk schema needs a migration and recovery policy.

## CI and artifacts

`.github/workflows/ci.yml` checks formatting and the declared MSRV, then builds Linux x86_64 release binaries. It packages `umc`, `umcd`, the MIT license, and install instructions in a tar archive and uploads the archive with a SHA-256 checksum for seven days. Windows and macOS are intentionally left out of the active matrix. The workflow does not publish a GitHub Release, sign binaries, or publish Rust crates.

Before creating a public release:

1. Document third-party notices for bundled dependencies and Mojang assets.
2. Review the RPC v1 contract, error mappings, and instance migration policy for the release.
3. Keep Linux install, daemon permissions, reconnect, launch, stop, and vanilla install behavior validated on a native runner.
4. Create a versioned Linux release archive from the verified workflow output. Add signing only after release key ownership and storage are defined.
5. Re-enable Windows and macOS CI/runtime validation when those ports are scheduled.

## License

UMC is licensed under MIT. The root `LICENSE` file is authoritative, and every workspace package inherits `license = "MIT"` and author metadata from the workspace manifest.
