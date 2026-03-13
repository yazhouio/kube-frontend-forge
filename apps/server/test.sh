#!/usr/bin/env bash
set -euo pipefail

SERVER_URL=${SERVER_URL:-http://127.0.0.1:3000}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
EXAMPLES_DIR="$SCRIPT_DIR/examples"
BUILD_JSON="$EXAMPLES_DIR/build.request.json"
PAGE_JSON="$EXAMPLES_DIR/page.schema.json"
MANIFEST_JSON="$EXAMPLES_DIR/manifest.test.json"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

for cmd in jq curl rg tar; do
  require_cmd "$cmd"
done

require_file() {
  if [[ ! -f "$1" ]]; then
    echo "Missing required file: $1" >&2
    exit 1
  fi
}

require_file "$BUILD_JSON"
require_file "$PAGE_JSON"
require_file "$MANIFEST_JSON"

post_json() {
  local url=$1
  local payload=$2
  local out=$3
  local code
  code=$(curl -sS -o "$out" -w "%{http_code}" \
    -H 'Content-Type: application/json' \
    --data-binary @"$payload" \
    "$SERVER_URL$url")
  if [[ "$code" != "200" ]]; then
    echo "HTTP $code for $url" >&2
    cat "$out" >&2
    exit 1
  fi
}

post_tar() {
  local url=$1
  local payload=$2
  local out=$3
  local code
  code=$(curl -sS -o "$out" -w "%{http_code}" \
    -H 'Content-Type: application/json' \
    --data-binary @"$payload" \
    "$SERVER_URL$url")
  if [[ "$code" != "200" ]]; then
    echo "HTTP $code for $url" >&2
    cat "$out" >&2
    exit 1
  fi
}

echo "==> /build"
post_json "/build" "$BUILD_JSON" "$TMP_DIR/build.out.json"
jq -e '.ok == true' "$TMP_DIR/build.out.json" >/dev/null
jq -r '.outputs.js.content' "$TMP_DIR/build.out.json" > "$TMP_DIR/build.js"
rg -q "System.register" "$TMP_DIR/build.js"

echo "==> /page/code"
post_json "/page/code" "$PAGE_JSON" "$TMP_DIR/page-code.out.json"
cat "$TMP_DIR/page-code.out.json"
jq -e '.ok == true' "$TMP_DIR/page-code.out.json" >/dev/null
jq -r '.code' "$TMP_DIR/page-code.out.json" > "$TMP_DIR/page-code.tsx"
rg -q "export default" "$TMP_DIR/page-code.tsx"

echo "==> /project/files"
post_json "/project/files" "$MANIFEST_JSON" "$TMP_DIR/project-files.out.json"
cat "$TMP_DIR/project-files.out.json"
jq -e '.ok == true' "$TMP_DIR/project-files.out.json" >/dev/null
jq -r '.files[] | select(.path == "src/index.ts") | .content' "$TMP_DIR/project-files.out.json" >/dev/null


echo "==> /project/files.tar.gz"
post_tar "/project/files.tar.gz" "$MANIFEST_JSON" "$TMP_DIR/project-files.tar.gz"
tar -tzf "$TMP_DIR/project-files.tar.gz" | rg -q '^src/index.ts$'


echo "==> /api/project/build"
post_json "/api/project/build" "$MANIFEST_JSON" "$TMP_DIR/project-build.out.json"
jq -e '.ok == true' "$TMP_DIR/project-build.out.json" >/dev/null
jq -r '.files[] | select(.path == "index.js") | .content' "$TMP_DIR/project-build.out.json" > "$TMP_DIR/project-build.js"
rg -q "System.register" "$TMP_DIR/project-build.js"


echo "==> /project/build.tar.gz"
post_tar "/project/build.tar.gz" "$MANIFEST_JSON" "$TMP_DIR/project-build.tar.gz"
tar -tzf "$TMP_DIR/project-build.tar.gz" | rg -q '^index.js$'


echo "All tests passed."
