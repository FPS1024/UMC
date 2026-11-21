# Optional services

UMC ships a small set of independently deployable companion services. None of them are required to
install or launch Minecraft; communities and launcher developers can run only the pieces they need.

UMC Auth and the initial UMC Mirror implementation have Docker Compose deployments. They can run
next to the launcher, behind HTTPS, or on separate hosts. The Java Edition server is planned and is
not packaged yet.

| Service | Status | Purpose |
|---|---|---|
| [UMC Auth](./auth) | Initial implementation | Account registration and Yggdrasil-compatible authentication |
| [Java Edition server](./java-server) | Planned | Dockerized Minecraft: Java Edition server |
| [Mirror](./mirror) | Initial implementation | HTTPS-only Go cache with SHA-1 verification for configured upstream objects |

## Design principles

- **Optional by default.** A vanilla UMC installation never contacts these services.
- **Independent deployment.** Implemented services have their own deployment configuration and data store.
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
