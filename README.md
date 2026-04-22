# Pallet

A standardized platform for syncing and placing AI agent configuration — rules,
skills, profiles, and prompts — from organizational sources to developer
workstations.

Pallet fetches resources from sources (hub, git repos, local directories) and
places them where each agent expects them — `.claude/rules/`, `.cursor/rules/`,
`.goose/memories/`, `.opencode/memories/`, `.codex/memories/` — read-only,
auditable, and hierarchically governed.

## Demo

![Pallet Demo](demo/demo.gif)

<details>
<summary>Regenerate the demo recording</summary>

Requires [VHS](https://github.com/charmbracelet/vhs): `brew install vhs`

```bash
./demo/record.sh
```

</details>

## The Problem

AI coding agents (Claude Code, Cursor, Goose, OpenCode, Codex) each have
their own configuration format and location. Organizations that want consistent
agent behavior across teams have no way to:

- Distribute coding standards, migration knowledge, and analysis profiles to
  developer workstations
- Enforce that governed resources can't be overridden locally
- Ensure agents have the right context regardless of which agent the developer uses
- Audit what configuration was active during any agent session

## How It Works

```
Sources                     Agent Placement                   Audit
(where to fetch)            (where agents read)               (what was synced)

hub API ─────┐              .claude/rules/, skills/, agents/
              ├──► merge ──► .cursor/rules/*.mdc, skills/     (direct write, 0444)
git repos ───┤              .goose/memories/, skills/
              │              .opencode/memories/, skills/
local dirs ──┘              .codex/memories/, skills/, agents/
                            pallet.lock                       (project root)
```

## Quick Start

```bash
# Authenticate to hub (saves credentials, creates pallet.yaml)
pallet auth https://hub.example.com --user you --password pass

# Or manually add a git source
pallet config add-source konveyor-skills --type git \
  --url https://github.com/konveyor/skills

# Sync and place for all detected agents
pallet sync .

# Preview what would be placed (no files written)
pallet sync . --dry-run

# Deterministic reproduction from lock file
pallet sync . --locked

# Re-sync from cache without pulling remotes
pallet lock .
```

## Configuration

### Project Config (`pallet.yaml`)

Lives in the project root. Committed to the repo. Defines what sources to sync.

```yaml
hub:
  url: https://hub.example.com

sources:
  - name: org-governance
    type: git
    url: https://github.com/org/governance

  - name: team-skills
    type: git
    url: https://github.com/team/skills
    paths:
      - skills/agent-readiness
      # Annotated path with conditional loading
      - path: rules/rust-conventions.md
        kind: rule
        globs: ["*.rs"]
        description: "Rust coding conventions"

  - name: hub-profiles
    type: hub

agents:
  auto_detect: true
```

### Credentials (`~/.pallet/credentials.yaml`)

Hub token stored locally, never committed.

### Lock File (`pallet.lock`)

Written to the project root after each sync. Records exact source refs,
content hashes, and placed paths. Serves as the audit trail — git history
provides the timeline.

## Layer Hierarchy

Sources are ordered — first = most authoritative (org-wide), last = most specific.
Local files in `.claude/` coexist alongside pallet-managed resources.

```
Source 0: org-governance     <- org level (most authoritative)
Source 1: team-skills        <- team level
Source 2: hub-profiles       <- hub (project-specific)
Local:    .claude/rules/*    <- project-specific (your files, not managed by pallet)
```

### Governance

Per-resource in frontmatter:
- `governed` — cannot be overridden by less-authoritative sources
- `federated` — can be overridden (default)

## Architecture

Pallet uses three adapter layers, each independently extensible:

- **Source adapters** — where to fetch (hub, git, local directory)
- **Resource adapters** — what to handle (rules, skills, agents, profiles, prompts)
- **Agent adapters** — where to place (Claude Code, Cursor, Goose, OpenCode, Codex)

Each agent adapter translates pallet's canonical format into agent-native
format. Skills use the [Agent Skills](https://agentskills.io) standard
(`SKILL.md`) supported by 30+ agents. Rules are placed as individual files
per agent — `.md` for Claude, `.mdc` for Cursor, plain markdown in
`memories/` for Goose/OpenCode/Codex.

See [docs/DESIGN.md](docs/DESIGN.md) for the full architecture.
See [docs/USE_CASES.md](docs/USE_CASES.md) for detailed use case walkthroughs.
See [docs/roadmap/](docs/roadmap/) for planned features.
