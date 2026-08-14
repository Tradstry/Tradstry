# DevOps

All deployment and infrastructure configuration lives here.

## Layout

- `compose.yml` — production service orchestration
- `docker/` — backend, SnapTrade, and Postgres images
- `caddy/` — reverse proxy and TLS routing
- `scripts/` — database cutover, cleanup, SSH hardening, and local Redis tools
- `bugsink/` — Bugsink environment template
- `Makefile` — local services, deployment, and release helpers

## Production deployment

Merges to `master` run `.github/workflows/release.yml`. The workflow verifies
the backend, builds immutable backend, MCP, and SnapTrade images tagged with the
full Git commit (`sha-<commit>`), and automatically deploys that exact commit to
`/opt/tradstry` over SSH.

Production secrets remain as ignored files on the server. GitHub stores only a
dedicated deployment SSH key and the pinned server host key. The deployment
script validates that the commit belongs to `origin/master`, serializes deploys
with a host lock, waits for Compose healthchecks, and restores the previous Git
commit and image tag when deployment fails.

The deployment public key is installed with an OpenSSH forced command pointing
to the root-owned `/usr/local/sbin/tradstry-deploy-command` wrapper, whose
versioned source is `scripts/ssh-deploy-command.sh`. It accepts only
`deploy <40-character-sha> sha-<same-sha>`, clears the SSH process environment,
and invokes `deploy-production.sh`. Interactive shells, forwarding, PTYs,
arbitrary commands, and commits outside `origin/master` are rejected. External
GitHub Actions are pinned to full commit SHAs, and repository settings enforce
SHA pinning.

`make deploy` is an emergency fallback for redeploying the already-built
`origin/master` images. `make tag` creates release metadata only and is not part
of production deployment.

GitHub Actions remain in `../.github/workflows` because GitHub only discovers
workflows from that location. The root `../Makefile` is a compatibility shim, so
existing commands such as `make backend` and `make deploy` still work.

## Compose

```bash
docker compose --env-file devops/.env -f devops/compose.yml config --quiet
docker compose --env-file devops/.env -f devops/compose.yml up -d
docker compose --env-file devops/.env -f devops/compose.yml logs -f
```

Production secrets are ignored by Git and live beside the Compose file:
`devops/.env`, `devops/countly.env`, `devops/countly-dashboard.env`, and
`devops/bugsink/.env.production`.
