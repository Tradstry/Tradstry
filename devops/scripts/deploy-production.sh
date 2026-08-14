#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

DEPLOY_DIR="${DEPLOY_DIR:-/opt/tradstry}"
COMMIT_SHA="${1:-}"
IMAGE_TAG="${2:-}"
LOCK_FILE="${DEPLOY_DIR}/.deploy.lock"
DEPLOY_ENV="${DEPLOY_DIR}/devops/deploy.env"
PREVIOUS_ENV="${DEPLOY_DIR}/devops/deploy.env.previous"

compose() {
  docker compose \
    --project-name tradstry \
    --env-file devops/.env \
    --env-file devops/deploy.env \
    -f devops/compose.yml \
    "$@"
}

if [[ ! "${COMMIT_SHA}" =~ ^[0-9a-f]{40}$ ]]; then
  echo "Error: deployment commit must be a full 40-character Git SHA." >&2
  exit 2
fi

if [[ "${IMAGE_TAG}" != "sha-${COMMIT_SHA}" ]]; then
  echo "Error: image tag must be sha-<deployment commit>." >&2
  exit 2
fi

cd "${DEPLOY_DIR}"

exec 9>"${LOCK_FILE}"
if ! flock -n 9; then
  echo "Error: another production deployment is already running." >&2
  exit 3
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: tracked files on the production checkout have local changes." >&2
  exit 4
fi

echo "Fetching deployment commit ${COMMIT_SHA}..."
git fetch --quiet origin master:refs/remotes/origin/master
git cat-file -e "${COMMIT_SHA}^{commit}"

if ! git merge-base --is-ancestor "${COMMIT_SHA}" origin/master; then
  echo "Error: ${COMMIT_SHA} is not reachable from origin/master." >&2
  exit 5
fi

PREVIOUS_COMMIT="$(git rev-parse HEAD)"
PREVIOUS_TAG="latest"
if [[ -f "${DEPLOY_ENV}" ]]; then
  saved_tag="$(sed -n 's/^DEPLOY_TAG=//p' "${DEPLOY_ENV}" | tail -1)"
  if [[ -n "${saved_tag}" ]]; then
    PREVIOUS_TAG="${saved_tag}"
  fi
  cp "${DEPLOY_ENV}" "${PREVIOUS_ENV}"
else
  printf 'DEPLOY_TAG=%s\n' "${PREVIOUS_TAG}" >"${PREVIOUS_ENV}"
fi
chmod 600 "${PREVIOUS_ENV}"

rollback() {
  status=$?
  trap - ERR
  echo "Deployment failed; restoring ${PREVIOUS_COMMIT} with ${PREVIOUS_TAG}..." >&2
  git checkout --quiet --detach "${PREVIOUS_COMMIT}" || true
  cp "${PREVIOUS_ENV}" "${DEPLOY_ENV}" || true
  chmod 600 "${DEPLOY_ENV}" || true
  compose up -d --remove-orphans --wait --wait-timeout 180 || true
  compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile || true
  exit "${status}"
}
trap rollback ERR

git checkout --quiet --detach "${COMMIT_SHA}"
printf 'DEPLOY_TAG=%s\n' "${IMAGE_TAG}" >"${DEPLOY_ENV}"
chmod 600 "${DEPLOY_ENV}"

echo "Validating production configuration..."
compose config --quiet

echo "Pulling immutable application images..."
compose pull backend mcp-server snaptrade-service

echo "Starting production services..."
compose up -d --remove-orphans --wait --wait-timeout 180

echo "Reloading Caddy..."
compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile

echo "Verifying application health..."
compose exec -T backend curl -sS -o /dev/null --max-time 8 http://localhost:7899/health
compose exec -T mcp-server curl -fsS --max-time 8 http://localhost:7900/health >/dev/null
compose exec -T snaptrade-service sh -c 'test -S /run/tradstry/snaptrade.sock && kill -0 1'

trap - ERR

printf '%s\n' "${PREVIOUS_COMMIT}" >"${DEPLOY_DIR}/.previous-deploy-commit"
printf '%s\n' "${COMMIT_SHA}" >"${DEPLOY_DIR}/.current-deploy-commit"
chmod 600 "${DEPLOY_DIR}/.previous-deploy-commit" "${DEPLOY_DIR}/.current-deploy-commit"

echo "Production deployment complete."
echo "Commit: ${COMMIT_SHA}"
echo "Images: ${IMAGE_TAG}"
compose ps
