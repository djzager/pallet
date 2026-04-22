#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PALLET_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Use provided binary path or default to release build
PALLET_BIN="${PALLET_BIN:-$PALLET_ROOT/target/release/pallet}"

if [[ ! -x "$PALLET_BIN" ]]; then
    echo "Error: pallet binary not found at $PALLET_BIN"
    echo "Run 'cargo build --release' first."
    exit 1
fi

# Create temp directory for the demo workspace
DEMO_TMPDIR="$(mktemp -d)"
export DEMO_TMPDIR
echo "Demo environment: $DEMO_TMPDIR"

# --- Create simulated project workspace ---
WORKSPACE="$DEMO_TMPDIR/workspace/pallet"
mkdir -p "$WORKSPACE"
(
    cd "$WORKSPACE"
    git init -b main --quiet
    git remote add origin https://github.com/konveyor/pallet.git
    mkdir -p .claude .cursor .goose
    echo "# Pallet" > README.md
    # Copy the real pallet.yaml from the project
    cp "$PALLET_ROOT/pallet.yaml" .
    git add .
    git commit -m "Initial commit" --quiet
)

# --- Write env file for the tape to source ---
cat > "$DEMO_TMPDIR/env.sh" <<EOF
export PALLET_BIN="$PALLET_BIN"
export WORKSPACE="$WORKSPACE"
EOF

echo ""
echo "Demo environment ready:"
echo "  Workspace: $WORKSPACE"
echo "  Env file:  $DEMO_TMPDIR/env.sh"
