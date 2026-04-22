# Pallet Demo — AI Agent Configuration Sync
#
# This is a VHS tape template. __DEMO_TMPDIR__ is replaced by record.sh
# with the actual temp directory path at recording time.

Output demo/demo.gif
Output demo/demo.mp4

# Terminal appearance
Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 700
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 40ms
Set WindowBar Colorful
Set WindowBarSize 40

# Hidden setup: source env, cd to workspace, alias pallet binary
Hide
Type "source __DEMO_TMPDIR__/env.sh && cd $WORKSPACE && alias pallet=$PALLET_BIN"
Enter
Sleep 500ms
Type "clear"
Enter
Sleep 300ms
Show

# --- Scene 1: Show committed pallet.yaml ---
Type "# Project config — two git sources, selective paths"
Enter
Sleep 800ms

Type "cat pallet.yaml"
Enter
Sleep 3s

# --- Scene 2: Preview with dry-run ---
Type "# Preview placement and context impact (no files written)"
Enter
Sleep 800ms

Type "pallet sync . --dry-run"
Enter
Sleep 15s

# --- Scene 3: Sync resources from GitHub ---
Type "# Sync and place for all detected agents"
Enter
Sleep 800ms

Type "pallet sync ."
Enter
Sleep 15s

# --- Scene 4: Inspect placed files across agents ---
Type "# Resources placed for Claude, Cursor, Goose, OpenCode, Codex"
Enter
Sleep 800ms

Type "tree .claude/agents .claude/skills .cursor/rules .goose/memories 2>/dev/null || true"
Enter
Sleep 3s

# --- Scene 5: Show a placed agent ---
Type "# Review a placed agent definition"
Enter
Sleep 800ms

Type "head -8 .claude/agents/00-engineering-toolkit-security-reviewer.md"
Enter
Sleep 3s

Sleep 1s
