# Optional UMC services

These services are independently deployable companions to the UMC launcher engine. Communities and
launcher developers can run only the services they need.

| Service | Status | Purpose |
|---|---|---|
| [`auth/`](./auth/) | Initial implementation | Self-hosted Go/PostgreSQL account registration and Yggdrasil-compatible authentication for Minecraft servers |
| [`java-server/`](./java-server/) | Planned | Dockerized Minecraft: Java Edition server |
| [`mirror/`](./mirror/) | Initial implementation | HTTPS-only Go object cache with SHA-1 verification |

The Auth and Mirror services currently provide Docker Compose deployments. The Java Edition server
is planned and is not packaged yet. Companion services are optional and are not required by `umc`
or `umcd`. UMC Auth provides community-operated Yggdrasil-compatible accounts; it does not provide
Microsoft sign-in or verify Minecraft ownership.
