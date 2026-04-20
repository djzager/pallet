# Keychain-based Credential Storage

## Status: Planned

## Problem

Pallet currently stores hub credentials in `~/.pallet/credentials.yaml` as
plain text (with 0600 permissions). This is functional but not ideal for
security-conscious environments.

## Proposed Solution

Follow the `gh auth` pattern: try the system keychain first, fall back to
plain text file storage.

### How `gh` does it

1. **Default: system keychain** — macOS Keychain, Windows Credential Manager,
   or Linux GNOME Keyring/KWallet (via D-Bus)
2. **Fallback: plain text config file** — if no keychain is available
   (headless/containers)
3. **`--insecure-storage` flag** — opt-in to skip keychain
4. **`GH_TOKEN` env var** — token without storing it at all

Key design decisions from `gh`:
- Keychain is the default; insecure storage is the fallback
- Token retrieval is via `gh auth token` command, not by reading files directly
- They use [go-keyring](https://github.com/zalando/go-keyring) under the hood

### Rust implementation options

- [`keyring`](https://crates.io/crates/keyring) crate — cross-platform
  (macOS Keychain, Windows Credential Manager, Linux Secret Service/D-Bus)
- On macOS, could also shell out to `security` CLI
- `PALLET_HUB_TOKEN` env var as an alternative to stored credentials

### Proposed behavior

```
pallet auth <hub_url> --user <user> --password <pass>
  1. Authenticate with hub API
  2. Try to store token in system keychain (service: "pallet", account: hub_url)
  3. If keychain unavailable, fall back to ~/.pallet/credentials.yaml (0600)
  4. Print which storage method was used

pallet sync .
  1. Try to read token from keychain
  2. If not found, try ~/.pallet/credentials.yaml
  3. If not found, try PALLET_HUB_TOKEN env var
  4. If none, error with "Run `pallet auth` first"
```

## References

- [gh auth login manual](https://cli.github.com/manual/gh_auth_login)
- [gh credential storage discussion](https://github.com/cli/cli/issues/1773)
- [gh keyring rollout discussion](https://github.com/cli/cli/discussions/7109)
- [keyring crate](https://crates.io/crates/keyring)
