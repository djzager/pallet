# Pallet Design

## Overview

Pallet is a sync-and-place engine for AI agent configuration. It fetches
resources from organizational sources and writes them directly to where each
agent expects them — read-only, auditable, and hierarchically governed.

The design is informed by Puppet's catalog compilation model, OPA's bundle
distribution, and the Terraform/Puppet adapter pattern.

## Core Concepts

### Resources

A resource is any file that governs agent behavior. Pallet treats all resource
types uniformly — they differ in content and placement, not in how they're
synced or governed.

| Type | What it governs | Example |
|------|----------------|---------|
| **rule** | How the agent behaves | "Use 2-space indentation", "Run npm test" |
| **skill** | What the agent knows | "Java EE to Quarkus migration plan" |
| **agent** | Agent persona/behavior | "Migration specialist agent" |
| **profile** | What the agent analyzes with | Analysis profile with rulesets |
| **prompt** | Workflow templates | "Create a migration plan from violations" |

Every resource has metadata in YAML frontmatter:

```markdown
---
name: security-baseline
type: rule
governance: governed
---
# Security Baseline
* Never commit secrets to source control
* All dependencies must come from approved registries
```

Optional frontmatter fields:

| Field | Effect |
|-------|--------|
| `governance` | `governed` (cannot be overridden) or `federated` (can be overridden, default) |
| `globs` | File patterns for conditional loading (e.g., `["*.rs"]`) |
| `paths` | Claude-native alias for `globs` |
| `description` | Human-readable description (used by Cursor for relevance matching) |

Resources without explicit governance default to `federated`. Resources with
`globs` or `description` are conditionally loaded — they don't count toward the
agent's context budget.

### Sources

A source is where resources come from. Pallet supports multiple source types
through source adapters:

| Type | How it fetches | Use case |
|------|---------------|----------|
| `hub` | Hub API (archetype -> profile bundle) | Managed analysis profiles |
| `git` | Clone/pull a repo | Governance repos with rules/skills |
| `local` | Read from a local directory | Local resources outside the project |

Sources are listed in `pallet.yaml` in the project root. Their order defines
the hierarchy (first = most general/authoritative, last = most specific).

### Agents

An agent is an AI coding tool that reads configuration from a specific location
in a specific format. Pallet detects which agents are present and places
resources directly where each expects them.

| Agent | Detection | Rules | Skills | Agents |
|-------|-----------|-------|--------|--------|
| Claude Code | `.claude/` dir or `claude` in PATH | `.claude/rules/*.md` (preserves frontmatter) | `.claude/skills/*/` (Agent Skills) | `.claude/agents/*.md` |
| Cursor | `.cursor/` dir or `cursor` in PATH | `.cursor/rules/*.mdc` (with `alwaysApply`/`globs` frontmatter) | `.cursor/skills/*/` (Agent Skills) | `.cursor/rules/*.mdc` |
| Goose | `.goose/` dir or `goose` in PATH | `.goose/memories/*.md` (plain markdown) | `.goose/skills/*/` (Agent Skills) | `.goose/memories/*.md` |
| OpenCode | `.opencode/` dir or `opencode` in PATH | `.opencode/memories/*.md` (plain markdown) | `.opencode/skills/*/` (Agent Skills) | `.opencode/memories/*.md` |
| Codex | `.codex/` dir or `codex` in PATH | `.codex/memories/*.md` (plain markdown) | `.codex/skills/*/` (Agent Skills) | `.codex/agents/*.md` |

Each adapter translates pallet's canonical resource format into the agent's
native format. For example, a rule with `globs: ["*.rs"]` becomes a `paths:`
frontmatter entry for Claude and an `alwaysApply: false` + `globs:` entry for
Cursor's `.mdc` format. Agents without conditional loading (Goose, OpenCode,
Codex) receive plain markdown with frontmatter stripped.

## Architecture

### Three Adapter Layers

```
Source Adapters           Resource Adapters          Agent Adapters
(where to fetch)          (what to handle)           (where to place)

hub   ──┐                 profile ──┐                claude   ──┐
git   ──┼──► fetch ──►    skill   ──┼──► merge ──►   cursor   ──┤
local ──┘                 rule    ──┤                 goose    ──┼──► write directly
                          agent   ──┤                 opencode ──┤    to agent dirs
                          prompt  ──┘                 codex    ──┘    (0444)
```

Each layer is independently extensible. Adding a new source type, resource
type, or agent is just adding an adapter. The core sync pipeline doesn't change.

### Sync Pipeline

`pallet sync .` executes this pipeline:

```
1. Load config
   Read pallet.yaml from project root
   Load credentials from ~/.pallet/credentials.yaml (if hub sources)

2. Collect facts
   Read git remote from workspace

3. Fetch from sources (in order)
   For each source in config.sources:
     Source adapter fetches resources
   In --locked mode: verify config hash matches lock file

4. Merge with hierarchy
   For each resource name across all sources:
     If first (highest authority) occurrence is governed -> use it, skip lower
     If first occurrence is federated -> use the last (most specific) occurrence

5. Context budget check
   For each detected agent:
     Sum always-loaded resources (rules, agents — excluding conditional)
     If over budget (~120KB / ~30K tokens):
       --dry-run: report only
       --force: warn and continue
       default: fail with guidance

6. Clean up previous placements
   Read pallet.lock from project root (if exists)
   Remove previously-placed files/directories

7. Place for agents
   For each detected agent:
     Translate resources to agent-native format
     Write to agent directories (0444)
     Track placed paths and content hashes

8. Write lock file
   Write pallet.lock to project root with:
     - Config hash, timestamp
     - Source resolved refs (git SHAs)
     - Per-resource hashes and placed paths

9. Context impact report
   Print per-agent breakdown of always-loaded vs on-demand resources
```

## Configuration

### Project Config (`pallet.yaml`)

Lives in the project root. Committed to the repo.

```yaml
hub:
  url: https://hub.example.com

# Sources in hierarchy order (first = most authoritative)
sources:
  - name: org-governance
    type: git
    url: https://github.com/org/governance

  - name: team-skills
    type: git
    url: https://github.com/team/skills
    paths:
      - skills/agent-readiness
      - path: rules/rust-conventions.md
        kind: rule
        globs: ["*.rs", "src/**/*.rs"]        # conditional loading
        description: "Rust coding conventions"

  - name: hub-profiles
    type: hub

agents:
  auto_detect: true
```

Path entries support two forms:

- **Simple**: `"skills/agent-readiness"` — just a path, kind inferred from directory name
- **Annotated**: `{ path, kind, globs, description }` — explicit kind and conditional loading metadata

When `globs` or `description` are set, the resource is treated as conditionally
loaded (not counted toward context budget). Each agent adapter translates these
to its native format:

- **Claude Code**: `globs` becomes `paths:` frontmatter
- **Cursor**: `globs` becomes `alwaysApply: false` + `globs:` in `.mdc` frontmatter
- **Goose/OpenCode/Codex**: no conditional loading mechanism — metadata is ignored

### Credentials (`~/.pallet/credentials.yaml`)

Hub authentication token. Stored with 0600 permissions. Never committed.

```yaml
hub_token: <token>
```

See [roadmap/keychain-credential-storage.md](roadmap/keychain-credential-storage.md)
for planned keychain-based secure storage.

### Local Resources

Files placed directly in `.claude/rules/`, `.claude/skills/`, etc. by the
developer coexist with pallet-managed resources. Pallet uses a naming convention
(`{NN}-{source}-{name}.md`) for its files and only cleans up files it placed
(tracked in `pallet.lock`). Your files are never touched.

## Hierarchy and Merge

### Source Order IS the Hierarchy

Sources in the config are ordered. First = most general (highest authority).
Last = most specific.

```
Source 0: org-governance        <- org level (most authoritative)
Source 1: team-skills           <- team level
Source 2: hub-profiles          <- hub (project-specific via archetype matching)
Local:    .claude/rules/*       <- project level (not managed by pallet)
```

### Merge Algorithm

```
merged = {}

for source in sources (order = authority, first = highest):
  for resource in source.resources:
    if resource.name not in merged:
      merged[resource.name] = resource
    else:
      existing = merged[resource.name]
      if existing.governance == "governed":
        # governed resource from a higher authority — skip this override
        log_warning("Cannot override governed resource: {}", resource.name)
      else:
        # federated resource — later (more specific) source wins
        merged[resource.name] = resource
```

## Lock File and Audit

Every sync writes `pallet.lock` to the project root:

```yaml
config_hash: sha256:abc123...
locked_at: 2026-04-20T15:00:00Z
sources:
  - name: org-governance
    type: git
    resolved_ref: abc123def456
    url: https://github.com/org/governance
resources:
  - kind: rule
    name: security-baseline
    source: org-governance
    source_index: 0
    governance: governed
    content_hash: sha256:deadbeef...
    placed_paths:
      - .claude/rules/00-org-governance-security-baseline.md
```

The lock file serves as:
- **Manifest**: what pallet placed (for cleanup on re-sync)
- **Audit trail**: what was synced and from where
- **Reproducibility**: `pallet sync . --locked` reproduces exact state

Git history of `pallet.lock` provides the full audit timeline.

## Context Budget

Each agent has a context budget (~120KB / ~30K tokens by default) representing
the maximum always-loaded content before performance degrades. Pallet estimates
context impact per agent and fails the sync if the budget is exceeded.

Resources are categorized as:

- **Always-loaded**: rules and agent definitions — loaded into context at every
  turn. These count toward the budget.
- **On-demand**: skills (Agent Skills format) — only the name and description
  are loaded at startup (~100 tokens each). Full content is loaded when the
  agent activates the skill.
- **Conditional**: rules with `globs` or `description` — loaded only when the
  agent is working on matching files. Not counted toward the budget.

The post-sync context impact report shows per-agent breakdown:

```
--- Context impact ---

  Claude Code:
    Always-loaded: 3 resource(s), ~12KB (~3000 tokens)
      org-governance: 2 resource(s), ~8KB
      team-skills: 1 resource(s), ~4KB
    On-demand: 5 resource(s), ~45KB (no startup cost)
    Budget: ~117KB (~29250 tokens)
```

Use `--dry-run` to preview without writing files, or `--force` to override
budget failures.

## Numeric Prefixing for Order

For agents that load all files from a directory (Claude Code's `.claude/rules/`),
files are prefixed with numbers to enforce hierarchy order:

```
.claude/rules/
├── 00-org-security-baseline.md       <- org level loaded first
├── 01-team-coding-standards.md       <- team level
├── 01-team-commit-messages.md
└── my-project-testing.md             <- local (not managed by pallet)
```

## Built-in Pallet Skill

Pallet embeds a self-awareness skill that is placed as an
[Agent Skill](https://agentskills.io) in each detected agent's skills directory
(e.g., `.claude/skills/pallet/SKILL.md`, `.cursor/skills/pallet/SKILL.md`).
This teaches the agent about pallet commands, resource locations, and the
governance model.

## CLI Commands

```
pallet auth <hub_url> --user USER --password PASS    Authenticate, save credentials, create config
pallet config show                                   Print current config
pallet config add-source NAME --type TYPE --url URL  Add a source
pallet config remove-source NAME                     Remove a source
pallet sync [PATH]                                   Fetch, merge, place, write lock
pallet sync --locked [PATH]                          Reproduce exact state from lock file
pallet sync --dry-run [PATH]                         Preview placement with context impact report
pallet sync --force [PATH]                           Continue even if context budget exceeded
pallet lock [PATH]                                   Re-sync from cache without pulling remotes
```

## Prior Art

| System | Pattern borrowed |
|--------|-----------------|
| **Puppet** | Catalog compilation, provider/type adapter pattern, cached catalogs |
| **OPA** | Bundle distribution, decision logs for auditability |
| **Terraform** | Provider adapter pattern |
| **EditorConfig** | Hierarchical file-based config with layered overrides |
| **chezmoi/GNU Stow** | File placement from a canonical store |
| **rulesync** | Per-agent format translation (`.mdc`, plain markdown, etc.) |
| **AGENTS.md** | Open standard for project instructions (Linux Foundation) |
| **Agent Skills** | On-demand skill loading via SKILL.md (agentskills.io) |
