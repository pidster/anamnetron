#!/usr/bin/env bash
set -euo pipefail

# Pre-commit hook: lint, format, type-check, test, and audit
# Install: ln -sf ../../.claude/hooks/pre-commit.sh .git/hooks/pre-commit

echo "Running pre-commit checks..."

# --- Rust checks ---

# Only run cargo checks if Cargo.toml exists (project is initialized)
if [ ! -f "Cargo.toml" ]; then
    echo "No Cargo.toml found, skipping cargo checks."
else
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
        echo "ERROR: Rust tests failed."
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
fi

# --- Web checks ---

if [ -d "web" ] && [ -f "web/package.json" ]; then
    echo "  web: npm test"
    (cd web && npm test) || {
        echo "ERROR: web tests failed."
        exit 1
    }

    echo "  web: svelte-check"
    (cd web && npx svelte-check --tsconfig ./tsconfig.json) || {
        echo "ERROR: svelte-check found type errors or warnings."
        exit 1
    }
else
    echo "No web/ directory found, skipping web checks."
fi

echo "All pre-commit checks passed."
