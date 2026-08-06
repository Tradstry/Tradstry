# DevOps

All deployment and infrastructure configuration lives here.

## Layout

- `compose.yml` — production service orchestration
- `docker/` — backend, SnapTrade, and Postgres images
- `caddy/` — reverse proxy and TLS routing
- `scripts/` — database cutover, cleanup, SSH hardening, and local Redis tools
- `bugsink/` — Bugsink environment template
- `Makefile` — local services, deployment, and release helpers

GitHub Actions remain in `../.github/workflows` because GitHub only discovers
workflows from that location. The root `../Makefile` is a compatibility shim, so
existing commands such as `make backend` and `make deploy` still work.

## Compose

```bash
docker compose --env-file devops/.env -f devops/compose.yml config --quiet
docker compose --env-file devops/.env -f devops/compose.yml up -d
docker compose --env-file devops/.env -f devops/compose.yml logs -f
```

Local production secrets are ignored by Git and live beside the Compose file:
`devops/.env`, `devops/countly.env`, `devops/countly-dashboard.env`, and
`devops/bugsink/.env.production`.
