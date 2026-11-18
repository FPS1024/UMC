---
translation: pending
source: en/guide/install.md
---

# Installation

UMC ships as a Linux x86_64 GNU archive containing two executables:

- `umc` — CLI and stdio JSON-RPC adapter.
- `umcd` — local JSON-RPC daemon over a private Unix socket.

## Download a release

Download the archive and its SHA-256 checksum from the
[latest GitHub release](https://github.com/FPS1024/UMC/releases/latest), then verify and unpack:

```sh
VERSION=0.1.0
BASE="https://github.com/FPS1024/UMC/releases/download/v${VERSION}"
curl -LO "${BASE}/umc-v${VERSION}-linux-x86_64-gnu.tar.gz"
curl -LO "${BASE}/umc-v${VERSION}-linux-x86_64-gnu.tar.gz.sha256"
sha256sum -c "umc-v${VERSION}-linux-x86_64-gnu.tar.gz.sha256"
tar -xzf "umc-v${VERSION}-linux-x86_64-gnu.tar.gz"
install -m 755 "umc-v${VERSION}-linux-x86_64-gnu/umc" ~/.local/bin/umc
install -m 755 "umc-v${VERSION}-linux-x86_64-gnu/umcd" ~/.local/bin/umcd
```

Confirm the binaries run:

```sh
umc help
```

::: warning Releases are unsigned

Verify the published SHA-256 checksum before unpacking or running a release. Releases are not
code-signed yet.

:::

## Build from source

Install Rust 1.89 or newer with [rustup](https://rustup.rs/), then build the workspace:

```sh
git clone https://github.com/FPS1024/UMC.git
cd UMC
cargo build --workspace --locked --release
```

The executables are written to `target/release/umc` and `target/release/umcd`. To install them
into the current Rust user's binary directory:

```sh
cargo install --path crates/umc-cli --locked
```

## Data directory

UMC stores instances, downloads, managed Java runtimes, task state, process state, and logs beneath
one data directory. Choose it with `UMC_DATA_DIR`:

```sh
export UMC_DATA_DIR="$HOME/.local/share/umc"
umc version list
```

If `UMC_DATA_DIR` is unset, development builds use `./umc-data` relative to the current working
directory. Keep this directory private: it contains the daemon socket and local process state.

## Uninstall

Remove the binaries and, if you no longer need your managed data, the data directory:

```sh
rm -f ~/.local/bin/umc ~/.local/bin/umcd
rm -rf "${UMC_DATA_DIR:-./umc-data}"
```

Removing the data directory deletes local instances, downloads, runtimes, logs, and task history.
It does not delete a Microsoft or Minecraft account.

## Next steps

- [Quickstart](/zh/guide/quickstart) — create and launch your first instance.
- [User guide](/zh/guide/user-guide) — commands, data layout, and network behavior.
