#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PALLET_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Pallet Demo Recording ==="

# --- Check dependencies ---
for cmd in vhs tree; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: '$cmd' not found. Install with: brew install $cmd"
        exit 1
    fi
done

# --- Build pallet ---
echo "Building pallet (release)..."
cd "$PALLET_ROOT"
cargo build --release --quiet
export PALLET_BIN="$PALLET_ROOT/target/release/pallet"
echo "Binary: $PALLET_BIN"

# --- Set up demo environment ---
echo "Setting up demo environment..."
source "$SCRIPT_DIR/setup.sh"
# setup.sh exports DEMO_TMPDIR and writes $DEMO_TMPDIR/env.sh

# --- Generate tape from template ---
echo "Generating tape file..."
GENERATED_TAPE="$DEMO_TMPDIR/demo.tape"
sed "s|__DEMO_TMPDIR__|$DEMO_TMPDIR|g" "$SCRIPT_DIR/demo.tape" > "$GENERATED_TAPE"

# --- Record ---
echo "Recording demo..."
cd "$PALLET_ROOT"
vhs "$GENERATED_TAPE"

# --- Cleanup ---
echo "Cleaning up demo environment..."
rm -rf "$DEMO_TMPDIR"

echo ""
echo "=== Done ==="
echo "Outputs:"
echo "  demo/demo.gif"
echo "  demo/demo.mp4"
