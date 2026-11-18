# Install UMC

The current release target is Linux x86_64. Windows and macOS are future platform ports. The release workflow publishes an unsigned, versioned Linux archive and SHA-256 checksum.

## Build from source

Install Rust 1.89 or newer with `rustup`, then from the root of a UMC checkout build the workspace:

```sh
git clone https://github.com/FPS1024/UMC.git
cd UMC
cargo build --workspace --locked --release
```

The executables are `target/release/umc` and `target/release/umcd` (`.exe` on Windows). Run them from that directory or install them into the current Rust user's binary directory:

```sh
cargo install --path crates/umc-cli --locked
```

This installs `umc` and `umcd` for the current user. Cargo manages the binaries under its configured bin directory.

## CI artifacts

For development builds from a branch or pull request, open the corresponding GitHub Actions run and download `umc-linux-x86_64`. Artifacts expire after seven days and should not be treated as stable releases. For a versioned build, download the Linux archive and `.sha256` checksum from the [GitHub Releases page](https://github.com/FPS1024/UMC/releases), verify the checksum, extract the tar archive, and place `umc` and `umcd` in a directory on `PATH`.

## First run

Set `UMC_DATA_DIR` to a directory where the current user can store game files, then run `umc version list`. Create and install a vanilla instance with:

```sh
umc instance create Survival 1.21.10
umc instance list
umc instance install <instance-id>
```

The install command prints its task ID and progress while it runs. Launching currently requires explicit offline mode:

```sh
umc launch <instance-id> Player --offline
```

`umcd` uses the same `UMC_DATA_DIR` as its clients. On Unix it creates a private local socket in that directory; on Windows it uses a local named pipe. Microsoft authentication and authenticated online play are not implemented.

For the Rust SDK, add a local path dependency while developing:

```toml
[dependencies]
umc-sdk = { path = "path/to/UMC/crates/umc-sdk" }
```

Publishing crates to crates.io is not configured yet. Set the canonical repository URL and prepare third-party notices before that step.
