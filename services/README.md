# Optional UMC services

These services are independently deployable companions to the UMC launcher engine. Communities and
launcher developers can run only the services they need.

| Service | Status | Purpose |
|---|---|---|
| [`auth/`](./auth/) | Initial implementation | Self-hosted Go/PostgreSQL account registration and Yggdrasil-compatible authentication for Minecraft servers |
| `mirror/` | Planned | Download mirror for Minecraft versions, libraries, assets, and Java runtimes |
| `skindesigner/` | Planned | Web skin editor and texture host |

Each service is packaged as its own Docker image and is not required by `umc` or `umcd`.
