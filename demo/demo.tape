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

# --- Scene 2: Sync resources from GitHub ---
Type "# Sync agents, skills, and Rust rules"
Enter
Sleep 800ms

Type "pallet sync ."
Enter
Sleep 15s

# --- Scene 3: Inspect placed files ---
Type "# Agents and skills placed for Claude Code"
Enter
Sleep 800ms

Type "tree .claude/agents .claude/skills"
Enter
Sleep 3s

# --- Scene 4: Show a placed agent ---
Type "# Review agents are ready to use"
Enter
Sleep 800ms

Type "head -8 .claude/agents/00-engineering-toolkit-security-reviewer.md"
Enter
Sleep 3s

Sleep 1s
