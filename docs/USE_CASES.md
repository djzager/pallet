# Use Cases

## Use Case 1: Managed Migrator

An application migrator who needs analysis profiles and migration skills to
perform a migration. Configuration is determined by the architect via the hub.

### Actors

- **Architect** — creates analysis profiles, migration skills, and agent rules
  in the hub
- **Migrator** — assigned to migrate a specific application. Consumes what the
  architect provides. Cannot modify governed resources.

### Flow

```
$ pallet auth https://hub.example.com --user migrator1 --password ***

Authenticating with hub: https://hub.example.com
Authenticated as: migrator1
Token expires: 1750000000
Hub connected: 12 applications visible
Credentials saved to ~/.pallet/credentials.yaml

Configuration written to pallet.yaml
Sources:
  - engineering-toolkit (git: https://github.com/org/toolkit)
    path: skills/agent-readiness
  - hub-profiles (hub profile sync)
```

```
$ pallet sync .

Loaded config (2 source(s))
Workspace: ~/Workspace/coolstore (branch: main, remote: github.com/acme-corp/coolstore)

Fetching source: engineering-toolkit (git)
  Fetched 1 resource(s) from 'engineering-toolkit'
  Resolved ref: abc123def456

Fetching source: hub-profiles (hub)
  Found profile: eap7-to-quarkus (id: 42)

Merging 2 resource(s) from 2 source(s)...
  2 resource(s) after merge

Cleaning up 0 previously-placed resource(s)...

Placing resources...
  Detected agent: Claude Code
    Built-in skill 'pallet': .claude/skills/pallet/SKILL.md
    Skill 'agent-readiness': .claude/skills/agent-readiness
    Profile 'eap7-to-quarkus' fetched (not placed for Claude — used by kantra)
  Lock file written to pallet.lock

Sync complete:
  Sources: engineering-toolkit, hub-profiles
  skills: 1
  profiles: 1
  Agents: claude
```

The migrator opens Claude Code. The agent automatically sees the migration
skills. The analysis profile is available for kantra.

---

## Use Case 2: Red Hat Employee Implementing Agentic SDLC

A trusted developer who uses AI agents across multiple repos. They want
consistent coding standards inherited from org and team levels, with
project-specific overrides. They own their configuration.

### Actors

- **Platform team** (RH Hybrid Platforms) — maintains org-wide governance repo
  with coding standards and security baselines
- **Team lead** (Konveyor) — maintains team governance repo with commit
  conventions, project-specific skills
- **Developer** — works across multiple repos, uses AI agents daily

### Flow

The developer's `pallet.yaml` in tackle2-ui:

```yaml
sources:
  - name: rh-hybrid-platforms
    type: git
    url: https://github.com/rh/governance

  - name: konveyor
    type: git
    url: https://github.com/konveyor/governance

agents:
  auto_detect: true
```

Syncing:

```
$ cd ~/Workspace/konveyor/tackle2-ui
$ pallet sync .

Loaded config (2 source(s))
Workspace: ~/Workspace/konveyor/tackle2-ui (branch: main, remote: github.com/konveyor/tackle2-ui)

Fetching source: rh-hybrid-platforms (git)
  Fetched 2 resource(s):
    rules/security-baseline.md       (governed)
    rules/coding-standards.md        (federated)

Fetching source: konveyor (git)
  Fetched 3 resource(s):
    rules/coding-standards.md        (federated, overrides rh-hybrid-platforms)
    rules/commit-messages.md         (federated)
    skills/konveyor-architecture/    (federated)

Merging 5 resource(s) from 2 source(s)...
  4 resource(s) after merge

Placing resources...
  Detected agent: Claude Code
    Built-in skill 'pallet': .claude/skills/pallet/SKILL.md
    Rule 'security-baseline': .claude/rules/00-rh-hybrid-platforms-security-baseline.md
    Rule 'coding-standards': .claude/rules/01-konveyor-coding-standards.md
    Rule 'commit-messages': .claude/rules/01-konveyor-commit-messages.md
    Skill 'konveyor-architecture': .claude/skills/konveyor-architecture
  Lock file written to pallet.lock

Sync complete:
  Sources: rh-hybrid-platforms, konveyor
  rules: 3
  skills: 1
  Agents: claude
```

The developer also has a local testing rule at `.claude/rules/testing.md`
(not managed by pallet). Both synced and local rules coexist.

### What the developer can do

- Modify `pallet.yaml` (add/remove sources, change agents)
- Add project-local resources directly in `.claude/` that coexist with
  synced resources from organizational sources
- Override federated resources from any source

### What the developer cannot do

- Override governed resources (security-baseline stays regardless)
- Modify pallet-placed files (0444 permissions)
