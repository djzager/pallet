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

# Create temp directory for the entire demo
DEMO_TMPDIR="$(mktemp -d)"
export DEMO_TMPDIR
echo "Demo environment: $DEMO_TMPDIR"

# --- 1. Create "org-governance" git repo ---
ORG_REPO="$DEMO_TMPDIR/repos/org-governance"
mkdir -p "$ORG_REPO"
cp -r "$SCRIPT_DIR/fixtures/org-governance/"* "$ORG_REPO/"
(
    cd "$ORG_REPO"
    git init -b main --quiet
    git add .
    git commit -m "Initial org governance rules" --quiet
)

# --- 2. Create "team-skills" git repo ---
TEAM_REPO="$DEMO_TMPDIR/repos/team-skills"
mkdir -p "$TEAM_REPO"
cp -r "$SCRIPT_DIR/fixtures/team-skills/"* "$TEAM_REPO/"
(
    cd "$TEAM_REPO"
    git init -b main --quiet
    git add .
    git commit -m "Initial team skills and rules" --quiet
)

# --- 3. Create simulated project workspace ---
WORKSPACE="$DEMO_TMPDIR/workspace/my-app"
mkdir -p "$WORKSPACE"
(
    cd "$WORKSPACE"
    git init -b main --quiet
    git remote add origin https://github.com/acme-corp/my-app.git
    mkdir -p .claude
    echo "# My App" > README.md
    git add .
    git commit -m "Initial commit" --quiet
)

# --- 4. Write env file for the tape to source ---
cat > "$DEMO_TMPDIR/env.sh" <<EOF
export PALLET_BIN="$PALLET_BIN"
export WORKSPACE="$WORKSPACE"
export ORG_REPO="file://$ORG_REPO"
export TEAM_REPO="file://$TEAM_REPO"
EOF

echo ""
echo "Demo environment ready:"
echo "  Workspace: $WORKSPACE"
echo "  Org repo:  $ORG_REPO"
echo "  Team repo: $TEAM_REPO"
echo "  Env file:  $DEMO_TMPDIR/env.sh"
