#!/usr/bin/env bash
# Regenerates the API contract and the generated Rust client (PLAN §7.6, D15).
#
#   api/generate.sh           rewrite api/openapi.json, api/rust-client/src/generated/ and the
#                             demo client used by strata-client's tests
#   api/generate.sh --check   write nothing; exit 1 if any of them is stale (CI)
#
# The generator formats its output (prettyplease + rustfmt with the workspace rustfmt.toml),
# so `cargo fmt` leaves the generated files unchanged.
set -euo pipefail

cd "$(dirname "$0")/.."

check=0
case "${1:-}" in
  "") ;;
  --check) check=1 ;;
  *) echo "usage: api/generate.sh [--check]" >&2; exit 2 ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

emit() { cargo run --quiet -p strata-api --features test-support --bin emit-openapi -- "$@"; }
codegen() { cargo run --quiet -p strata-codegen -- "$@"; }

# 1. The contract (the only JSON artefact) and the demo contract (test fixture input only).
emit "$tmp/openapi.json"
emit --demo "$tmp/demo-openapi.json"

status=0
if [[ $check -eq 1 ]]; then
  if ! cmp -s "$tmp/openapi.json" api/openapi.json; then
    echo "api/openapi.json is stale" >&2
    status=1
  fi
else
  cp "$tmp/openapi.json" api/openapi.json
fi

# 2. The generated client, and the demo client exercised by strata-client's tests.
mode=()
[[ $check -eq 1 ]] && mode=(--check)
codegen --input "$tmp/openapi.json" --output api/rust-client/src/generated \
  --core-path crate --source-label api/openapi.json "${mode[@]}" || status=1
codegen --input "$tmp/demo-openapi.json" --output api/rust-client/tests/demo/generated \
  --core-path ::strata_client --source-label "the strata-api demo contract" "${mode[@]}" || status=1

if [[ $status -ne 0 ]]; then
  echo "generated API artefacts are stale: run api/generate.sh" >&2
fi
exit $status
