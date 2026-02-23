#!/usr/bin/env bash
#
# Build all components of Anamnetron in dependency order:
#
#   1. WASM   — wasm-pack compiles crates/wasm → crates/wasm/pkg/
#   2. Web    — Vite bundles web/ (consumes WASM pkg) → web/dist/
#   3. Rust   — cargo builds the workspace (server serves web/dist/)
#
# Usage:
#   ./scripts/build.sh           # full build (all three stages)
#   ./scripts/build.sh --release # release profile for Rust + WASM
#   ./scripts/build.sh wasm      # only WASM
#   ./scripts/build.sh web       # only web (assumes WASM pkg exists)
#   ./scripts/build.sh rust      # only Rust workspace
#
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

PROFILE="dev"
WASM_PROFILE=""
STAGES=()

for arg in "$@"; do
  case "$arg" in
    --release)
      PROFILE="release"
      WASM_PROFILE="--release"
      ;;
    wasm|web|rust)
      STAGES+=("$arg")
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      echo "Usage: $0 [--release] [wasm] [web] [rust]" >&2
      exit 1
      ;;
  esac
done

# Default: build everything
if [ ${#STAGES[@]} -eq 0 ]; then
  STAGES=(wasm web rust)
fi

CARGO_PROFILE_FLAG=""
if [ "$PROFILE" = "release" ]; then
  CARGO_PROFILE_FLAG="--release"
fi

step() {
  echo ""
  echo "==> $1"
  echo ""
}

for stage in "${STAGES[@]}"; do
  case "$stage" in
    wasm)
      step "Building WASM (crates/wasm → crates/wasm/pkg/)"
      wasm-pack build crates/wasm --target web $WASM_PROFILE --no-pack
      ;;
    web)
      step "Building web frontend (web/ → web/dist/)"
      if [ ! -d "crates/wasm/pkg" ]; then
        echo "Warning: crates/wasm/pkg/ not found. Run 'wasm' stage first." >&2
      fi
      cd web
      npm run build
      cd "$ROOT_DIR"
      ;;
    rust)
      step "Building Rust workspace"
      cargo build --workspace $CARGO_PROFILE_FLAG
      ;;
  esac
done

echo ""
echo "Build complete."
