#!/bin/sh
#
# Install git hooks for Discrakt development
#
# Usage: ./.githooks/install.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Installing git hooks..."

# Configure git to use the .githooks directory
git config core.hooksPath .githooks

echo "Git hooks installed successfully!"
echo ""
echo "The following hooks are now active:"
echo "  - pre-commit: runs cargo fmt --check and clippy"
