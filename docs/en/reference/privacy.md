# Privacy Policy

_Last updated: November 19, 2026_

This Privacy Policy explains what UMC (Universal Minecraft Core) collects, sends, and stores. It applies to the UMC Core library, command-line tools, daemon, SDK, loader adapters, documentation, and official builds. It does not cover third-party services such as Microsoft, Xbox, Minecraft, mod platforms, or the servers you join; those services have their own policies.

## Summary

- UMC does not operate a user account service, telemetry endpoint, analytics pipeline, advertising system, crash-reporting service, cloud sync, or remote management service.
- UMC stores game files, instances, runtimes, task state, and logs locally on your device.
- Microsoft sign-in happens on Microsoft's pages. UMC never receives your Microsoft password.
- The current version does not persist Microsoft or Minecraft tokens to disk. Tokens exist in process memory only and are discarded when the command exits.
- Version lookup, installation, and loader metadata requests go directly to the relevant upstream services, which may receive ordinary connection metadata such as your IP address.

## Data stored on your device

UMC keeps its data under `UMC_DATA_DIR` (or `./umc-data` when developing). This can include:

| Data | Purpose |
|---|---|
| Instance configuration and metadata | Records the name, Minecraft version, memory settings, loader selection, and installation state of each instance. |
| Game files, assets, libraries, and Java runtimes | Downloaded from Mojang and other upstream sources so the game can run locally. |
| Task and process records | Lets the daemon report progress, recover interrupted work, and track running games. |
| Logs | Captures installation progress, launch output, and errors for troubleshooting. |
| Version and metadata cache | Avoids repeated upstream requests. |

UMC does not store your Microsoft password. It also does not store Microsoft or Minecraft tokens in the current version.

## Microsoft account sign-in

When you run `umc auth login`, the following happens:

1. UMC starts a temporary HTTP listener on the loopback interface and opens Microsoft's authorization page in your browser. With `--no-browser`, UMC prints the URL instead.
2. You sign in on Microsoft's site, including any consent, account-selection, or multi-factor prompts. UMC never sees your credentials.
3. Microsoft redirects your browser to the loopback address with a one-time authorization code. UMC checks the `state` value and uses PKCE (S256) to bind the request to this sign-in attempt.
4. UMC exchanges the authorization code for Microsoft tokens at Microsoft's token endpoint.
5. UMC sends the Microsoft access token to Xbox Live user authentication, XSTS authorization, and Minecraft services login, and receives Xbox and XSTS tokens plus a Minecraft access token.
6. UMC requests the Minecraft profile (username, UUID, skins, and capes) to confirm the account.

The requested scope is `XboxLive.signin offline_access`. `offline_access` allows Microsoft to return a refresh token; the current version receives it but does not save it.

Account data handled during sign-in:

| Data | Current handling |
|---|---|
| Microsoft password | Never seen or stored by UMC. Entered only on Microsoft's pages. |
| Microsoft access token | Kept in memory for the duration of the sign-in command; not written to disk. |
| Microsoft refresh token | Received but discarded when the process exits; not written to disk. |
| Xbox Live and XSTS tokens | Kept in memory during the login chain; not written to disk. |
| Minecraft access token | Kept in memory; not written to disk. |
| Minecraft profile (name, UUID, skins, capes) | Displayed in the terminal and held in memory for the command; not persisted. |

A future version is expected to store the refresh token in OS credential storage so you can sign in without reopening the browser. When that happens, this policy will describe the storage location, removal command, and sign-out behavior before the feature ships.

## Network requests

UMC makes HTTPS requests directly from your device to upstream services. It does not route them through a UMC-operated proxy.

| Purpose | Typical destination |
|---|---|
| Minecraft version manifest and game metadata | `piston-meta.mojang.com`, `launchermeta.mojang.com` |
| Game files, assets, and libraries | Mojang and related CDN hosts |
| Java runtime downloads | Mojang runtime metadata and download hosts |
| Loader metadata and installer downloads | Fabric, Forge, and NeoForge endpoints |
| Microsoft sign-in and token exchange | `login.microsoftonline.com` |
| Xbox Live and XSTS authorization | `user.auth.xboxlive.com`, `xsts.auth.xboxlive.com` |
| Minecraft services login and profile | `api.minecraftservices.com` |

These services may receive ordinary connection metadata, including your IP address, request time, requested URL, user agent, and any token or account information required by that request. Their own privacy policies apply.

## Local daemon and IPC

The UMC daemon listens only on local IPC endpoints (a Unix socket or a Windows named pipe) and does not expose a network port. Any local process that can connect to that endpoint can control UMC, so protect the data directory and its permissions. UMC does not authenticate local clients beyond operating-system access control.

## Logs and diagnostics

Logs may contain:

- player names, Minecraft UUIDs, and account names;
- local file paths and instance names;
- game, Java, and mod output, including server addresses or chat printed by the game;
- error messages returned by upstream services.

Tokens are not written to logs. The token types used by UMC redact their contents when formatted for debugging and are not serialized to JSON. Review logs before sharing them publicly, and remove anything private.

## What UMC does not do

- No telemetry, analytics, usage statistics, or advertising identifiers.
- No crash reporting or automatic upload of logs.
- No cloud sync, remote backup, or account service operated by UMC.
- No selling or sharing of personal data.
- No background requests beyond the installation, version, loader, Java, and sign-in operations you initiate.

## Retention and deletion

Local data remains until you delete it:

- Delete an instance in UMC or remove its directory to delete that instance's files, settings, and logs.
- Delete `UMC_DATA_DIR` to remove all UMC-managed data.
- Tokens are discarded when the sign-in process exits, so there is nothing token-related to delete in the current version.
- Uninstalling UMC does not necessarily delete `UMC_DATA_DIR`; remove it yourself if you want a clean removal.

Data sent to Microsoft, Mojang, or other third parties is retained by those services under their own policies.

## Third-party services

Microsoft, Xbox, Minecraft, Mojang, Fabric, Forge, NeoForge, mod platforms, and multiplayer servers are independent services. Their terms and privacy policies govern the data they collect. UMC cannot delete data held by them or change their account settings for you.

## Changes to this policy

This policy will be updated when UMC adds account persistence, telemetry, hosted services, crash reporting, mod distribution, or other material data flows. The date at the top shows the last change, and material changes will be noted in the release notes.

## Contact

Privacy questions can be raised in the project's [GitHub issue tracker](https://github.com/FPS1024/UMC/issues).
