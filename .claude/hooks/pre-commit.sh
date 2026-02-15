#!/usr/bin/env bash
set -euo pipefail

# Pre-commit hook: lint, format, and audit checks
# Install: ln -sf ../../.claude/hooks/pre-commit.sh .git/hooks/pre-commit

echo "Running pre-commit checks..."

# Only run cargo checks if Cargo.toml exists (project is initialized)
if [ ! -f "Cargo.toml" ]; then
    echo "No Cargo.toml found, skipping cargo checks."
    exit 0
fi

echo "  cargo fmt --check"
cargo fmt --check || {
    echo "ERROR: cargo fmt check failed. Run 'cargo fmt' to fix."
    exit 1
}

echo "  cargo clippy"
cargo clippy --all-targets --all-features -- -D warnings || {
    echo "ERROR: cargo clippy found warnings."
    exit 1
}

echo "  cargo test"
cargo test --all-targets --all-features || {
    echo "ERROR: tests failed."
    exit 1
}

echo "  cargo audit"
if command -v cargo-audit &> /dev/null; then
    cargo audit || {
        echo "ERROR: cargo audit found vulnerabilities."
        exit 1
    }
else
    echo "  cargo-audit not installed, skipping. Install with: cargo install cargo-audit"
fi

echo "All pre-commit checks passed."
