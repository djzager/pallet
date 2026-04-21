# Pallet Demo — AI Agent Configuration Sync
#
# This is a VHS tape template. __DEMO_TMPDIR__ is replaced by record.sh
# with the actual temp directory path at recording time.

Output demo/demo.gif
Output demo/demo.mp4

# Terminal appearance
Set Shell "bash"
Set FontSize 16
Set Width 1000
Set Height 650
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

# --- Scene 1: Add organizational sources ---
Type "# Add org-wide governance rules"
Enter
Sleep 800ms

Type "pallet config add-source org-governance --type git --url $ORG_REPO"
Enter
Sleep 1.5s

Type "# Add team-level skills and overrides"
Enter
Sleep 800ms

Type "pallet config add-source team-skills --type git --url $TEAM_REPO"
Enter
Sleep 1.5s

# --- Scene 2: Show config ---
Type "pallet config show"
Enter
Sleep 2s

# --- Scene 3: Sync resources ---
Type "# Sync resources from all sources"
Enter
Sleep 800ms

Type "pallet sync ."
Enter
Sleep 3s

# --- Scene 4: Inspect placed files ---
Type "# Inspect what pallet placed for Claude Code"
Enter
Sleep 800ms

Type "tree .claude/"
Enter
Sleep 2s

# --- Scene 5: Show governance in action ---
Type "# Governed rule — cannot be overridden by downstream sources"
Enter
Sleep 800ms

Type "head -5 .claude/rules/00-org-governance-security-baseline.md"
Enter
Sleep 2s

Type "# Team's coding-standards override the org's federated version"
Enter
Sleep 800ms

Type "head -5 .claude/rules/01-team-skills-coding-standards.md"
Enter
Sleep 2s

Sleep 1s
