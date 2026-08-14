#!/usr/bin/env bash

set -Eeuo pipefail

readonly DEPLOY_DIR="/opt/tradstry"
readonly REQUESTED_COMMAND="${SSH_ORIGINAL_COMMAND:-}"

deny() {
  echo "Access denied: this key can only deploy an exact commit from master." >&2
  exit 64
}

if [[ ! "${REQUESTED_COMMAND}" =~ ^deploy[[:space:]]([0-9a-f]{40})[[:space:]](sha-[0-9a-f]{40})$ ]]; then
  deny
fi

readonly COMMIT_SHA="${BASH_REMATCH[1]}"
readonly IMAGE_TAG="${BASH_REMATCH[2]}"

if [[ "${IMAGE_TAG}" != "sha-${COMMIT_SHA}" ]]; then
  deny
fi

exec env -i \
  HOME="/home/tradstry-deploy" \
  LOGNAME="tradstry-deploy" \
  PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" \
  USER="tradstry-deploy" \
  DEPLOY_DIR="${DEPLOY_DIR}" \
  "${DEPLOY_DIR}/devops/scripts/deploy-production.sh" \
  "${COMMIT_SHA}" \
  "${IMAGE_TAG}"
