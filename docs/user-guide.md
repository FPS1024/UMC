# UMC User Guide

UMC is a headless engine for managing and launching Minecraft: Java Edition. It currently provides vanilla installation and explicit offline launch. Microsoft account authentication and authenticated online play are not implemented. Fabric, Forge, and NeoForge support currently stops at version discovery and installation preflight plans.

## Install and first run

Follow [Install UMC](install.md) to build or download the Linux x86_64 release. Set `UMC_DATA_DIR` to select where UMC stores managed data. If unset, development builds use `./umc-data` relative to the current working directory.

Create and install a vanilla instance:

```sh
umc version list
umc instance create Survival 1.21.10
umc instance list
umc instance install <instance-id>
umc task get <task-id>
```

Launch currently requires explicit offline mode:

```sh
umc launch <instance-id> Player --offline
```

Offline mode does not verify account ownership or enable authenticated multiplayer. Do not treat it as a Microsoft account login.

## Local data and logs

UMC stores instances, downloads, managed Java runtimes, task state, process state, and logs beneath the selected data directory. Game output is written to each instance's `logs/umc-console.log`. Logs can contain player names, local paths, Java output, or information printed by the game and mods; review them before sharing.

Back up the data directory before manually editing or deleting files. Use UMC commands to remove instances and runtimes where possible, so running-process checks can be applied. Removing the data directory deletes the local managed data; it does not delete a Microsoft or Minecraft account.

## Network behavior

Version lookup, game installation, and Java runtime installation make HTTPS requests to Mojang/Minecraft download infrastructure. Loader metadata requests go to Fabric Meta, Forge Maven, or NeoForge Maven as appropriate. UMC does not operate a proxy, account service, or telemetry endpoint in the current version. These remote services may receive ordinary connection metadata such as the client's IP address.

`umcd` accepts local IPC requests from clients sharing the configured data directory. Keep that directory private. UMC does not expose a network listener for remote control.

## Accounts and privacy

Microsoft authentication is not currently available, and this version does not store Microsoft passwords, access tokens, or refresh tokens. When account support is implemented, the project will document the browser flow, token storage, account removal, and the requests made to Microsoft, Xbox, and Minecraft services before enabling it.

See [Project, Trademark, and Privacy Notes](legal-and-privacy.md) for the current privacy summary and project ownership boundaries.
