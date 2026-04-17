# Pallet Design

## Overview

Pallet is a sync-and-place engine for AI agent configuration. It fetches
resources from organizational sources, stores them canonically in `.konveyor/`,
and symlinks them to where each agent expects them — read-only, auditable, and
hierarchically governed.

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
| `file` | Read a local directory | Project-local `.konveyor/` |

Sources are listed in the user config. Their order defines the hierarchy (first
= most general/authoritative, last = most specific). The workspace's
`.konveyor/` directory is implicitly the final source.

### Agents

An agent is an AI coding tool that reads configuration from a specific location
in a specific format. Pallet detects which agents are present and places
resources where each expects them.

| Agent | Detection | Placement |
|-------|-----------|-----------|
| Claude Code | `.claude/` dir or `claude` in PATH | Symlink to `.claude/rules/`, `.claude/skills/` |
| Cursor | `.cursor/` dir | Generate `.cursor/rules/*.mdc` |
| Goose | `.goose/` dir or `goose` in PATH | Symlink SKILL.md (native format) |
| OpenCode | `.opencode/` dir | Symlink to `.opencode/rules/` |
| Copilot | `.github/` dir | Generate `.github/copilot-instructions.md` |

Agent detection follows Puppet's `confine`/`defaultfor` pattern — each adapter
declares what filesystem markers or binaries indicate the agent is in use.

## Architecture

### Three Adapter Layers

```
Source Adapters           Resource Adapters          Agent Adapters
(where to fetch)          (what to handle)           (where to place)

hub  ──┐                  profile ──┐                claude  ──┐
git  ──┼──► fetch ──►     skill   ──┼──► store ──►   cursor  ──┼──► symlink/generate
file ──┘                  rule    ──┤    .konveyor/  goose   ──┤    to agent dirs
oci  ──  (future)         prompt  ──┘                opencode──┤    (0444)
                          ???     ──  (future)       copilot ──┘
```

Each layer is independently extensible. Adding a new source type, resource
type, or agent is just adding an adapter. The core sync pipeline doesn't change.

### Adapter Interfaces (Rust)

```rust
/// Source adapter — fetches resources from a remote or local source
trait SourceAdapter {
    /// What source type this adapter handles (e.g., "hub", "git", "file")
    fn source_type(&self) -> &str;

    /// Fetch all resources from this source
    fn fetch(&self, config: &SourceConfig, workspace: &Path) -> Result<Vec<RawResource>>;

    /// Check if updates are available (for caching/polling)
    fn has_updates(&self, config: &SourceConfig, last_sync: &SyncState) -> Result<bool>;
}

/// Resource adapter — parses, validates, and stores a resource type
trait ResourceAdapter {
    /// What resource kind this adapter handles (e.g., "rule", "skill", "profile")
    fn kind(&self) -> &str;

    /// Parse raw content into a typed resource
    fn parse(&self, raw: &RawResource) -> Result<Resource>;

    /// Validate a parsed resource
    fn validate(&self, resource: &Resource) -> Result<()>;

    /// Canonical storage path within .konveyor/
    fn storage_path(&self, resource: &Resource) -> PathBuf;
}

/// Agent adapter — places resources where a specific agent reads them
trait AgentAdapter {
    /// Agent name (e.g., "claude", "cursor", "goose")
    fn name(&self) -> &str;

    /// Detect if this agent is present in the workspace
    fn detect(&self, workspace: &Path) -> bool;

    /// Place a resource where this agent expects it
    fn place(&self, resource: &Resource, workspace: &Path) -> Result<Placement>;

    /// Remove a previously placed resource
    fn remove(&self, placement: &Placement) -> Result<()>;

    /// List all placements this adapter has made
    fn list_placements(&self, workspace: &Path) -> Result<Vec<Placement>>;
}
```

### Sync Pipeline

`pallet sync .` executes this pipeline:

```
1. Load config
   Read ~/.konveyor/config.yaml
   If managed: true, verify config integrity (hash check)

2. Collect facts
   Read git remote from workspace
   Detect installed agents (confine/defaultfor)

3. Fetch from sources (in order)
   For each source in config.sources:
     Source adapter fetches resources
     Resource adapters parse and validate each item

4. Merge with hierarchy
   For each resource name across all sources:
     If first (highest authority) occurrence is governed -> use it, skip lower
     If first occurrence is federated -> use the last (most specific) occurrence
   Workspace .konveyor/ resources are the final (most specific) layer

5. Store canonically
   Write merged resources to .konveyor/{kind}/{name}
   Set file permissions to 0444

6. Place for agents
   For each detected agent:
     Agent adapter creates symlinks or generates files
     Set permissions to 0444

7. Audit
   Append sync event to .konveyor/audit.jsonl
   Record: timestamp, sources fetched, resources placed, hashes, agents
```

## Configuration

### User Config (`~/.konveyor/config.yaml`)

Follows the user across all projects. Contains hub credentials, source list,
and agent preferences.

```yaml
# Managed mode (set by hub during login, immutable)
managed: true  # or false

# Hub connection (populated by pallet auth login)
hub:
  url: https://hub.example.com
  token: <token>

# Sources in hierarchy order (first = most authoritative)
sources:
  - name: rh-hybrid-platforms
    type: git
    url: https://github.com/rh/governance

  - name: konveyor
    type: git
    url: https://github.com/konveyor/governance

  - name: hub-profiles
    type: hub

# Agent configuration
agents:
  auto_detect: true
  # Or explicit:
  # enabled: [claude, goose]
```

### Project Config (workspace `.konveyor/`)

The workspace's `.konveyor/` directory is implicitly the final (most specific)
source. No config file needed — the directory structure IS the config:

```
.konveyor/
├── rules/
│   └── testing.md          # project-specific rule
├── skills/
│   └── custom-framework/
│       └── SKILL.md         # project-specific skill
└── prompts/
    └── review-checklist.md  # project-specific prompt
```

Resources here can override federated resources from higher sources but cannot
override governed resources.

### Managed vs Autonomous

The `managed` flag in the config determines the operating mode. It is set during
`pallet auth login` based on the user's role in the hub.

| Concern | Managed | Autonomous |
|---------|---------|------------|
| Who owns the config | Hub / organization | Developer |
| Config file permissions | 0444 (read-only) | 0644 (read-write) |
| Can add/remove sources | No | Yes |
| Can modify synced resources | No | No (0444 regardless) |
| Can add local resources | Only if org allows | Yes |
| Can override governed resources | No | No |
| Can override federated resources | No | Yes |

## Hierarchy and Merge

### Source Order IS the Hierarchy

Sources in the config are ordered. First = most general (highest authority).
Last = most specific. The workspace `.konveyor/` is always last.

```
Source 0: rh-hybrid-platforms    <- org level (most authoritative)
Source 1: konveyor               <- team level
Source 2: hub-profiles           <- hub (project-specific via archetype matching)
Source 3: workspace .konveyor/   <- project level (most specific, implicit)
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

### Governance Per Resource

Governance is declared per-resource in frontmatter, not per-source. A single
source can contain both governed and federated resources:

```
rh-hybrid-platforms/rules/
├── security-baseline.md       governance: governed   <- cannot be overridden
└── coding-standards.md        governance: federated  <- can be overridden
```

This allows an organization to lock security-critical rules while leaving
stylistic preferences flexible.

### Example Merge

```
Layer 0 (rh-hybrid-platforms):
  rules/security-baseline.md     (governed)     "never commit secrets"
  rules/coding-standards.md      (federated)    "2-space indent, PascalCase"

Layer 1 (konveyor):
  rules/coding-standards.md      (federated)    "2-space indent, camelCase"
  rules/commit-messages.md       (federated)    "conventional commits"

Layer 2 (workspace .konveyor/):
  rules/testing.md               (local)        "run npm test"

Result:
  rules/security-baseline.md     <- Layer 0 (governed, locked)
  rules/coding-standards.md      <- Layer 1 (overrode Layer 0's federated version)
  rules/commit-messages.md       <- Layer 1
  rules/testing.md               <- Layer 2
```

## Auth and Login

### The Login Flow

`pallet auth login` is the classification event. It authenticates the user and
the hub responds with role information and a recommended (or mandated) config.

```
pallet auth login --hub <url>
  1. Prompt for credentials (or accept --user/--password/--token)
  2. Authenticate to hub API
  3. Hub returns:
     - User role (migrator, architect, admin)
     - Config mode (managed or autonomous)
     - Source list (mandated for managed, recommended for autonomous)
  4. Write ~/.konveyor/config.yaml
     - If managed: write as 0444, user cannot modify
     - If autonomous: write as 0644, user can modify
  5. Store credentials securely (token in config or OS keychain)
```

This follows the Puppet model: the agent presents its identity (credentials),
the server classifies the node (user role + recommended sources), and the client
receives its configuration (the config file).

### Offline Behavior

After initial login and first sync, pallet works offline:
- The config persists in `~/.konveyor/config.yaml`
- Synced resources persist in `.konveyor/`
- Symlinks persist in agent directories
- `pallet sync .` warns if sources are unreachable but does not remove
  previously synced resources (cached catalog pattern from Puppet)

## Agent Placement

### Symlinks vs Generation

For agents that read standard markdown from a directory (Claude Code, Goose,
OpenCode), pallet creates symlinks from the agent's expected location to the
canonical `.konveyor/` store.

For agents that need a different format (Cursor's `.mdc`, Copilot's single
concatenated file), pallet generates the file from the canonical store content.

### Read-Only Enforcement

All placed files (symlinks and generated files) are set to 0444. The canonical
store in `.konveyor/` is also 0444 for synced (non-local) resources.

This means:
- Agents can read the config but cannot modify it
- Editors show read-only indicators
- Drift detection is simple: if permissions changed, someone tampered

### Numeric Prefixing for Order

For agents that load all files from a directory (Claude Code's `.claude/rules/`),
files are prefixed with numbers to enforce hierarchy order:

```
.claude/rules/
├── 00-rh-security-baseline.md       <- org level loaded first
├── 01-konveyor-coding-standards.md  <- team level
├── 02-konveyor-commit-messages.md
└── 03-tackle2-ui-testing.md         <- project level loaded last
```

## Audit Log

Every sync writes an entry to `.konveyor/audit.jsonl`:

```jsonl
{"ts":"2026-04-17T10:30:00Z","event":"sync","sources":["rh-hybrid-platforms","konveyor"],"resources":{"rules":4,"skills":2,"profiles":1,"prompts":1},"agents":["claude"],"hashes":{"security-baseline.md":"sha256:a1b2...","coding-standards.md":"sha256:c3d4..."}}
{"ts":"2026-04-17T10:30:01Z","event":"place","agent":"claude","resource":"security-baseline.md","method":"symlink","path":".claude/rules/00-rh-security-baseline.md"}
```

Drift detection (via `pallet status .`) checks:
- File permissions (should be 0444)
- Content hashes (should match last sync)
- Symlink targets (should point to `.konveyor/`)

```jsonl
{"ts":"2026-04-17T11:00:00Z","event":"drift","resource":"coding-standards.md","type":"permission_changed","expected":"0444","actual":"0644"}
{"ts":"2026-04-17T11:00:00Z","event":"drift","resource":"coding-standards.md","type":"content_modified","expected_hash":"sha256:c3d4...","actual_hash":"sha256:e5f6..."}
```

## CLI Commands

```
pallet auth login [--hub URL]           Authenticate to hub, receive/create config
pallet auth logout                      Remove stored credentials

pallet config show                      Print current config
pallet config edit                      Open config in editor (autonomous only)
pallet config add-source NAME           Add a source (autonomous only)
pallet config remove-source NAME        Remove a source (autonomous only)

pallet sync [PATH]                      Fetch, merge, store, place, audit
pallet sync --dry-run [PATH]            Show what would change without doing it

pallet status [PATH]                    Show placed resources, detect drift

pallet audit [PATH]                     Query the audit log
```

## Prior Art

| System | Pattern borrowed |
|--------|-----------------|
| **Puppet** | Catalog compilation (server classifies client, compiles config). Provider/type adapter pattern for agent placement. Managed vs masterless modes. Cached catalogs for offline. Idempotent convergence. |
| **OPA** | Bundle distribution (fetch, verify, apply). Decision logs for auditability. Service/credential separation for sources. |
| **Terraform** | Provider adapter pattern (same interface, different implementations). Schema-driven config validation. |
| **EditorConfig** | Hierarchical file-based config with layered overrides. |
| **MDM (Intune/Jamf)** | Managed vs unmanaged device distinction. Server decides what client gets based on group membership. |
| **ESLint shared configs** | Versioned, overridable organizational defaults via package distribution. |
| **chezmoi/GNU Stow** | Symlink-based file placement from a canonical store. |

## Implementation Plan

### Phase 1: POC (Prove the mechanism)

**Goal**: A Rust binary that syncs from a git source and places for Claude Code.

- `pallet sync .` with a single git source
- Resource types: rules, skills
- Agent adapter: Claude Code only
- Store in `.konveyor/`, symlink to `.claude/rules/` and `.claude/skills/`
- Read-only permissions (0444)
- Basic audit log (append-only JSONL)

**Deliverables**:
- `konveyor/skills` repo with starter rules and skills
- `pallet` binary (Rust)
- Demo: two repos with same team rules, different project rules

### Phase 2: Hub Integration

- `pallet auth login` with hub authentication
- Hub source adapter (fetch profiles via archetype matching)
- Managed vs autonomous mode based on hub role
- Resource type: profiles (alongside rules/skills)

### Phase 3: Multi-Agent and Hierarchy

- Additional agent adapters (Cursor, Goose, OpenCode)
- Governed vs federated merge semantics
- Multiple git sources with hierarchy ordering
- Agent auto-detection (confine/defaultfor)

### Phase 4: Enterprise Features

- Drift detection and reporting (`pallet status`)
- Content hashing and integrity verification
- Agent hooks integration (auto-sync before agent sessions)
- OCI registry as a source type
