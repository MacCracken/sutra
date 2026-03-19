# Dependency Watch

Tracking known upstream advisories and dependency issues that affect sutra.

## Active

### RUSTSEC-2023-0071 — rsa: Marvin Attack timing sidechannel

| Field | Value |
|-------|-------|
| Advisory | [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071) |
| Severity | 5.9 (medium) |
| Crate | `rsa` 0.9.10 |
| Dependency chain | `russh` → `ssh-key` → `rsa` |
| Fix available | No |
| Logged | 2026-03-19 |

**Description**: Potential RSA private key recovery through timing sidechannels during decryption operations.

**Impact on sutra**: Low. Sutra's SSH transport defaults to ed25519 keys. RSA keys are supported as a fallback but not recommended. The vulnerability requires an attacker to observe many decryption operations with precise timing — unlikely in an infrastructure orchestration context where SSH sessions are infrequent.

**Mitigation**: Prefer ed25519 keys (`ssh_key` field in inventory or `~/.ssh/id_ed25519` auto-detect). Avoid RSA keys on untrusted networks.

**Action**: Waiting on upstream fix in `rsa` crate. Monitor `russh` releases for a version that upgrades or replaces the `rsa` dependency.

## Resolved

(None yet)
