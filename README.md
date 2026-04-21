# Pallet

A standardized platform for syncing and placing AI agent configuration — rules,
skills, profiles, and prompts — from organizational sources to developer
workstations.

Pallet fetches resources from sources (hub, git repos) and writes them directly
to where each agent expects them (`.claude/`, `.cursor/`, `.goose/`, etc.) —
read-only, auditable, and hierarchically governed.

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

AI coding agents (Claude Code, Cursor, Goose, OpenCode, Copilot) each have
their own configuration format and location. Organizations that want consistent
agent behavior across teams have no way to:

- Distribute coding standards, migration knowledge, and analysis profiles to
  developer workstations
- Enforce that governed resources can't be overridden locally
- Ensure agents have the right context regardless of which agent the developer uses
- Audit what configuration was active during any agent session

## How It Works

```
Sources                     Agent Placement            Audit
(where to fetch)            (where agents read)        (what was synced)

hub API ─────┐
              ├──► merge ──► .claude/rules/     (direct write, 0444)
git repos ───┘              .claude/skills/    (direct write, 0444)
                            .claude/agents/    (direct write, 0444)
                            pallet.lock        (project root)
```

## Quick Start

```bash
# Authenticate to hub (saves credentials, creates pallet.yaml)
pallet auth https://hub.example.com --user you --password pass

# Or manually add a git source
pallet config add-source konveyor-skills --type git \
  --url https://github.com/konveyor/skills

# Sync and place for detected agents
pallet sync .

# Deterministic reproduction from lock file
pallet sync . --locked
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

- **Source adapters** — where to fetch (hub, git, local directory, OCI in future)
- **Resource adapters** — what to handle (profiles, skills, rules, prompts, future types)
- **Agent adapters** — where to place (Claude, Cursor, Goose, OpenCode, Copilot)

See [docs/DESIGN.md](docs/DESIGN.md) for the full architecture.
See [docs/USE_CASES.md](docs/USE_CASES.md) for detailed use case walkthroughs.
See [docs/roadmap/](docs/roadmap/) for planned features.
