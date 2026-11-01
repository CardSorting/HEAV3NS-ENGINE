#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [[ -f "${ROOT_DIR}/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "${ROOT_DIR}/.env"
  set +a
fi

UNITY_PATH="${UNITY_PATH:-/Applications/Unity/Hub/Editor/6000.6.7f1/Unity.app/Contents/MacOS/Unity}"
PROJECT_PATH="${ROOT_DIR}/unity_client"
EXECUTE_METHOD="${1:-HeadlessCliRunner.CompileAndVerify}"

# Ensure Bee/Stevedore cache directories are explicitly set for headless/sandboxed environments
export STEVEDORE_CACHE_FOLDER="${PROJECT_PATH}/Library/StevedoreCache"
export XDG_CACHE_HOME="${PROJECT_PATH}/Library/StevedoreCache"
export LOCALAPPDATA="${PROJECT_PATH}/Library/StevedoreCache"
mkdir -p "${STEVEDORE_CACHE_FOLDER}"

exec "${UNITY_PATH}" \
  -batchmode \
  -nographics \
  -projectPath "${PROJECT_PATH}" \
  -executeMethod "${EXECUTE_METHOD}" \
  -quit \
  -logFile -

