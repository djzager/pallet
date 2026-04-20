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

The `governance` field controls override behavior:
- `governed` — cannot be overridden by lower-priority sources
- `federated` — can be overridden by lower-priority sources (default)

Resources without explicit governance metadata default to `federated`.

### Sources

A source is where resources come from. Pallet supports multiple source types
through source adapters:

| Type | How it fetches | Use case |
|------|---------------|----------|
| `hub` | Hub API (archetype -> profile bundle) | Managed analysis profiles |
| `git` | Clone/pull a repo | Governance repos with rules/skills |

Sources are listed in `pallet.yaml` in the project root. Their order defines
the hierarchy (first = most general/authoritative, last = most specific).

### Agents

An agent is an AI coding tool that reads configuration from a specific location
in a specific format. Pallet detects which agents are present and places
resources directly where each expects them.

| Agent | Detection | Placement |
|-------|-----------|-----------|
| Claude Code | `.claude/` dir or `claude` in PATH | Direct write to `.claude/rules/`, `.claude/skills/`, `.claude/agents/` |
| Cursor | `.cursor/` dir | Generate `.cursor/rules/*.mdc` (future) |
| Goose | `.goose/` dir or `goose` in PATH | Direct write to `.goose/skills/` (future) |

## Architecture

### Three Adapter Layers

```
Source Adapters           Resource Adapters          Agent Adapters
(where to fetch)          (what to handle)           (where to place)

hub  ──┐                  profile ──┐                claude  ──┐
git  ──┼──► fetch ──►     skill   ──┼──► merge ──►   cursor  ──┼──► write directly
       │                  rule    ──┤                 goose   ──┤    to agent dirs
       │                  agent   ──┤                           │    (0444)
       │                  prompt  ──┘                           │
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
   Detect installed agents

3. Fetch from sources (in order)
   For each source in config.sources:
     Source adapter fetches resources

4. Merge with hierarchy
   For each resource name across all sources:
     If first (highest authority) occurrence is governed -> use it, skip lower
     If first occurrence is federated -> use the last (most specific) occurrence

5. Clean up previous placements
   Read pallet.lock from project root (if exists)
   Remove previously-placed files/directories

6. Place for agents
   For each detected agent:
     Write resources directly to agent directories
     Set permissions to 0444
     Track placed paths and content hashes

7. Write lock file
   Write pallet.lock to project root with:
     - Config hash, timestamp
     - Source resolved refs (git SHAs)
     - Per-resource hashes and placed paths
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

  - name: hub-profiles
    type: hub

agents:
  auto_detect: true
```

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

Pallet embeds a self-awareness skill that is always placed at
`.claude/skills/pallet/SKILL.md`. This teaches the agent about pallet commands,
resource locations, and the governance model.

## CLI Commands

```
pallet auth <hub_url> --user USER --password PASS    Authenticate, save credentials, create config
pallet config show                                   Print current config
pallet config add-source NAME --type TYPE --url URL  Add a source
pallet config remove-source NAME                     Remove a source
pallet sync [PATH]                                   Fetch, merge, place, write lock
pallet sync --locked [PATH]                          Reproduce exact state from lock file
```

## Prior Art

| System | Pattern borrowed |
|--------|-----------------|
| **Puppet** | Catalog compilation, provider/type adapter pattern, cached catalogs |
| **OPA** | Bundle distribution, decision logs for auditability |
| **Terraform** | Provider adapter pattern |
| **EditorConfig** | Hierarchical file-based config with layered overrides |
| **chezmoi/GNU Stow** | File placement from a canonical store |
