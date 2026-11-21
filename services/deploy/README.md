# Shared deployment assets

Optional deployment templates shared by UMC services.

UMC Auth and UMC Mirror each include their own Compose deployment. Auth uses Go, PostgreSQL, and
Caddy; Mirror uses a Go object cache and Caddy with automatic HTTPS. See [`../auth/`](../auth/) and
[`../mirror/`](../mirror/) for their separate configuration and data volumes.
