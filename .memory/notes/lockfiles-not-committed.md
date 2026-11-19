---
name: lockfiles-not-committed
description: The Cargo and pnpm lockfiles are not in the repo yet, so CI and Docker resolve dependencies fresh on every run
type: gotcha
source: docs/08-development.md:37, .github/workflows/ci.yml:76, commit 4f91f0c
verified: 2026-11-19
volatility: high
---

`docs/08-development.md` says generated lockfiles (Cargo.lock, pnpm-lock.yaml) should be
committed, but as of commit 4f91f0c neither exists in the repo. CI installs with
`pnpm install --no-frozen-lockfile` (`.github/workflows/ci.yml:76`, `:98`) to cope with that.

**Symptom:** a green build can turn red with no code change when an upstream crate or npm package
publishes a new version; local and CI dependency trees can differ.
**Avoid by:** committing both lockfiles once generated, then switching CI to `--frozen-lockfile`.
When that lands, delete this note.
