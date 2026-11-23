# Optional UMC services

These services are independently deployable companions to the UMC launcher engine. Communities and
launcher developers can run only the services they need.

| Service | Status | Purpose |
|---|---|---|
| [UMC-Auth](https://github.com/UMC-Community/UMC-Auth) | Account system implemented | Self-hosted Go/Nginx/PostgreSQL accounts with `/api/v1`, a Vue panel, optional Stalwart mail, and Yggdrasil-compatible authentication |
| [`java-server/`](./java-server/) | Planned | Dockerized Minecraft: Java Edition server |
| [`mirror/`](./mirror/) | Initial implementation | HTTPS-only Go object cache with SHA-1 verification |

The Auth and Mirror services currently provide Docker Compose deployments. UMC Auth uses Nginx, a
private Go auth API, and PostgreSQL, with an optional self-hosted Stalwart mail profile and a Vue
user center/admin panel. The Java
Edition server is planned and is not packaged yet. Companion services are optional and are not
required by `umc` or `umcd`. UMC Auth provides community-operated Yggdrasil-compatible accounts; it
does not provide Microsoft sign-in or verify Minecraft ownership.
