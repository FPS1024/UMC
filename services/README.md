# Optional UMC services

These services are independently deployable companions to the UMC launcher engine. Communities and
launcher developers can run only the services they need.

| Service | Status | Purpose |
|---|---|---|
| [`auth/`](./auth/) | Initial implementation | Self-hosted Go/PostgreSQL account registration and Yggdrasil-compatible authentication for Minecraft servers |
| `java-server/` | Planned | Dockerized Minecraft: Java Edition server |
| `mirror/` | Planned | Download mirror for Minecraft versions, libraries, assets, and Java runtimes |

The Auth service currently provides a Docker Compose deployment. The Java Edition server and Mirror
are planned and are not packaged yet. Companion services are optional and are not required by `umc`
or `umcd`. UMC Auth provides community-operated Yggdrasil-compatible accounts; it does not provide
Microsoft sign-in or verify Minecraft ownership.
