# Use Cases

## Use Case 1: Managed Migrator

An untrusted application migrator who needs analysis profiles and migration
skills to perform a migration. They have no ownership of the configuration —
it's determined by the architect via the hub.

### Actors

- **Architect** — creates analysis profiles, migration skills, and agent rules
  in the hub. Associates them with archetypes via target platforms.
- **Migrator** — assigned to migrate a specific application. Consumes what the
  architect provides. Cannot modify governed resources.

### Flow

```
$ pallet auth login --hub https://hub.example.com
Username: migrator1
Password: ***

Authenticating... done.

  Role: migrator
  Mode: managed (read-only configuration)
  Sources:
    - hub-profiles (hub: hub.example.com) [profiles]
    - konveyor-skills (git: github.com/konveyor/skills) [skills, rules, prompts]

Config written to ~/.konveyor/config.yaml (0444)
```

The hub classified this user as a migrator and returned a locked config. The
migrator cannot modify it. The sources, their order, and their governance modes
are all hub-determined.

```
$ cd ~/Workspace/coolstore
$ pallet sync .

Reading config: ~/.konveyor/config.yaml (managed)
Workspace: ~/Workspace/coolstore

Source: hub-profiles (hub)
  Identifying application from git remote...
    remote: github.com/acme-corp/coolstore
    application: coolstore
    archetype: Java EE Monoliths -> Quarkus 3.x
  Fetching profile bundle...
    profiles/eap7-to-quarkus/profile.yaml    (governed)
    profiles/eap7-to-quarkus/rules/          (governed)

Source: konveyor-skills (git)
  Fetching github.com/konveyor/skills@main...
    skills/java-ee-to-quarkus/SKILL.md       (governed)
    skills/reactive-messaging/SKILL.md       (governed)
    rules/migration-workflow.md              (governed)
    prompts/migration-plan.md                (governed)

Storing to .konveyor/:
  profiles/eap7-to-quarkus/     ok
  skills/java-ee-to-quarkus/    ok
  skills/reactive-messaging/    ok
  rules/migration-workflow.md   ok
  prompts/migration-plan.md     ok

Detecting agents... found: claude
Placing for claude:
  .claude/rules/migration-workflow.md    -> .konveyor/rules/migration-workflow.md    (0444)
  .claude/skills/java-ee-to-quarkus/    -> .konveyor/skills/java-ee-to-quarkus/    (0444)
  .claude/skills/reactive-messaging/    -> .konveyor/skills/reactive-messaging/    (0444)

Audit: 1 profile, 2 skills, 1 rule, 1 prompt synced. All governed.
```

The migrator opens Claude Code. The agent automatically sees the migration
skills and rules. The analysis profile is available for kantra. The migrator
didn't configure anything — they authenticated and synced.

### What the migrator cannot do

- Modify `~/.konveyor/config.yaml` (file is 0444)
- Add or remove sources
- Edit synced resources (symlink targets are 0444)
- Override governed resources with local `.konveyor/` files

### What the migrator can do

- Run `pallet sync .` in different projects (gets different profiles per project
  based on archetype matching, same skills across projects)
- View what's placed: `pallet status .`
- Use the synced resources via their agent of choice

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

```
$ pallet auth login --hub https://hub.example.com
Username: djzager
Password: ***

Authenticating... done.

  Role: architect
  Mode: autonomous (you own your configuration)

  Hub recommends the following sources:
    [0] rh-hybrid-platforms (git: github.com/rh/governance)         [governed]
    [1] konveyor            (git: github.com/konveyor/governance)   [federated]
    [2] hub-profiles        (hub: hub.example.com)                  [governed]

  Apply recommended configuration? [Y/n]: y

Config written to ~/.konveyor/config.yaml
Customize with: pallet config edit
```

The resulting config:

```yaml
# ~/.konveyor/config.yaml (owned by developer, read-write)
managed: false

hub:
  url: https://hub.example.com
  token: <token>

sources:
  - name: rh-hybrid-platforms
    type: git
    url: https://github.com/rh/governance

  - name: konveyor
    type: git
    url: https://github.com/konveyor/governance

  - name: hub-profiles
    type: hub

agents:
  auto_detect: true
```

Syncing in tackle2-ui:

```
$ cd ~/Workspace/konveyor/tackle2-ui
$ pallet sync .

Reading config: ~/.konveyor/config.yaml (autonomous)
Workspace: ~/Workspace/konveyor/tackle2-ui

Source: rh-hybrid-platforms (git)
  Fetching github.com/rh/governance@main...
    rules/security-baseline.md       (governed)
    rules/coding-standards.md        (federated)

Source: konveyor (git)
  Fetching github.com/konveyor/governance@main...
    rules/coding-standards.md        (federated, overrides rh-hybrid-platforms)
    rules/commit-messages.md         (federated)
    skills/konveyor-architecture/    (federated)

Source: hub-profiles (hub)
  No matching application found for github.com/konveyor/tackle2-ui
  (skipping hub profiles)

Workspace: .konveyor/
    rules/testing.md                 (local: "Run npm test before finalizing")

Merging (4 layers):
  security-baseline.md      <- rh-hybrid-platforms  (governed, locked)
  coding-standards.md       <- konveyor             (overrode org federated default)
  commit-messages.md        <- konveyor             (federated)
  testing.md                <- workspace            (local)

Detecting agents... found: claude, goose
Placing for claude (.claude/rules/):
  00-rh-security-baseline.md       -> symlink (0444)
  01-konveyor-coding-standards.md  -> symlink (0444)
  02-konveyor-commit-messages.md   -> symlink (0444)
  03-tackle2-ui-testing.md         -> symlink (0444)

Placing for goose:
  ...

Audit: 0 profiles, 1 skill, 4 rules synced.
```

Switching to tackle2-hub:

```
$ cd ~/Workspace/konveyor/tackle2-hub
$ pallet sync .

  ...same org + team rules...

Workspace: .konveyor/
    rules/testing.md                 (local: "Run make test before finalizing")

Merging:
  security-baseline.md      <- rh-hybrid-platforms  (governed, locked)
  coding-standards.md       <- konveyor             (overrode org federated default)
  commit-messages.md        <- konveyor             (federated)
  testing.md                <- workspace            (local, DIFFERENT from tackle2-ui)

Placing for claude (.claude/rules/):
  00-rh-security-baseline.md       -> symlink (0444)
  01-konveyor-coding-standards.md  -> symlink (0444)
  02-konveyor-commit-messages.md   -> symlink (0444)
  03-tackle2-hub-testing.md        -> symlink (0444)
```

Same org and team rules. Different project-level testing rule. The agent
inherits the right rules automatically in each repo.

### What the developer can do

- Modify `~/.konveyor/config.yaml` (add/remove sources, change agents)
- Add project-local resources in `.konveyor/` that extend or override federated
  resources from higher levels
- Override federated resources from any source

### What the developer cannot do

- Override governed resources (security-baseline stays regardless)
- Modify the content of synced resources from other sources (symlinks are 0444,
  targets in the store are 0444)

### The demo

1. Show the hierarchy configured (org -> team -> project)
2. Open tackle2-ui: agent has org + konveyor + tackle2-ui rules
3. Open tackle2-hub: agent has org + konveyor + tackle2-hub rules
4. Show that tackle2-ui doesn't get tackle2-hub's "run make test" rule
5. Change a konveyor-level rule -> re-sync -> both repos pick it up
6. Try to override a governed rule -> pallet warns and keeps the governed version
