# Mirror

The UMC Mirror is a planned optional service that will cache or proxy the files a launcher needs:

- Mojang version manifests and version metadata
- Client JARs
- Libraries
- Assets
- Java runtime downloads
- Loader metadata and installer artifacts

## Status

Not implemented yet. This page describes the intended scope so launcher integrations can plan for
it.

The current `umc` engine downloads directly from Mojang and the relevant loader services and verifies
files against their published SHA-1 hashes.

## Intended behavior

- Run as its own Docker image with an HTTPS reverse proxy.
- Accept an upstream base URL and a local cache directory.
- Verify every cached object against the upstream hash before serving it.
- Preserve the upstream directory layout so clients can switch between direct and mirrored URLs.
- Expose a small health and statistics endpoint.
- Never bundle Minecraft game files in the image.
- Support operator-configured retention and cache size limits.

## Non-goals for the first version

- User accounts
- Mod or modpack hosting
- Launcher authentication
- Rewriting or modifying game files

A mirror should be a transparent cache, not a distribution platform for content the operator is not
allowed to share.

## Relationship to UMC Auth

The mirror is independent from [UMC Auth](./auth). It does not require accounts and does not verify
players. A community can run either service, both, or neither.
