# Optional services

UMC ships a small set of independently deployable companion services. None of them are required to
install or launch Minecraft; communities and launcher developers can run only the pieces they need.

Each service is packaged as its own Docker image and can be placed next to the launcher, behind the
same HTTPS reverse proxy, or on a separate host.

| Service | Status | Purpose |
|---|---|---|
| [UMC Auth](./auth) | Initial implementation | Account registration and Yggdrasil-compatible authentication |
| [Java Edition server](./java-server) | Planned | Dockerized Minecraft: Java Edition server |
| [Mirror](./mirror) | Planned | Download mirror for versions, libraries, assets, and Java runtimes |

## Design principles

- **Optional by default.** A vanilla UMC installation never contacts these services.
- **Independent deployment.** Each service has its own Docker image, configuration, and data store.
- **HTTPS first.** Public services are expected to run behind a TLS-terminating reverse proxy.
- **Community-owned.** Accounts, downloads, and textures stay under the operator's control.
- **Small surface area.** Services do one job and expose a documented API rather than a large
  management platform.

## Choosing what to run

- Run **UMC Auth** if players should register accounts on your own site and join Minecraft servers
  through authlib-injector or another Yggdrasil-compatible launcher.
- Run **Java Edition server** if you want a managed Docker server for a community.
- Run **Mirror** if players in your community have slow or unreliable access to Mojang download
  hosts.

The [UMC Auth guide](./auth) covers deployment, the admin UI, invites, optional email verification,
Turnstile, proof-of-work, rate limits, and the HTTPS deployment check.
