# Pallet

A standardized platform for syncing and placing AI agent configuration — rules,
skills, profiles, and prompts — from organizational sources to developer
workstations.

Pallet fetches resources from sources (hub, git repos), stores them canonically
in `.konveyor/`, and symlinks them to where each agent expects them (`.claude/`,
`.cursor/`, `.goose/`, etc.) — read-only, auditable, and hierarchically
governed.

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
Sources                     Canonical Store           Agent Placement
(where to fetch)            (single truth)            (where agents read)

hub API ─────┐
              ├──► .konveyor/ ──────┬──► .claude/rules/     (symlink, 0444)
git repos ───┘     ├── profiles/    ├──► .claude/skills/    (symlink, 0444)
                   ├── skills/      ├──► .cursor/rules/     (generated, 0444)
                   ├── rules/       ├──► .goose/skills/     (symlink, 0444)
                   └── prompts/     └──► .opencode/rules/   (symlink, 0444)
```

## Quick Start

```bash
# Authenticate to hub (determines managed vs autonomous mode)
pallet auth login --hub https://hub.example.com

# Add a git source for skills and rules
pallet config add-source konveyor-skills --type git \
  --url https://github.com/konveyor/skills

# Sync and place for detected agents
pallet sync .

# Check what's placed and detect drift
pallet status .
```

## Two Operating Modes

**Managed** — for untrusted migrators. `pallet auth login` returns a read-only
config from the hub. The migrator can't change sources or modify synced
resources. The hub decides what they get based on their application's archetype.

**Autonomous** — for trusted developers. `pallet auth login` returns a
recommended config that the developer can customize. They own their config, add
sources, choose agents. Governed resources from higher-authority sources still
can't be overridden.

## Architecture

Pallet uses three adapter layers, each independently extensible:

- **Source adapters** — where to fetch (hub, git, local directory, OCI in future)
- **Resource adapters** — what to handle (profiles, skills, rules, prompts, future types)
- **Agent adapters** — where to place (Claude, Cursor, Goose, OpenCode, Copilot)

Adding support for a new source, resource type, or agent is just adding an
adapter. The core sync pipeline doesn't change.

See [docs/DESIGN.md](docs/DESIGN.md) for the full architecture.
See [docs/USE_CASES.md](docs/USE_CASES.md) for detailed use case walkthroughs.
