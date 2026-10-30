<!--
  Title must be a Conventional Commit, e.g.
  feat(proto): reuse kitty placements between frames
  fix(pacer): reset clock after seek
-->

## What and why

<!-- One paragraph. Link the issue: "Closes #123". -->

Closes #

## How it was tested

<!-- Commands, fixtures, terminals used (name + version), platform(s). -->

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features --locked`
- [ ] Manual playback in: <!-- kitty / WezTerm / Windows Terminal / iTerm2 / Alacritty -->

## Performance

<!-- Required for anything touching the hot path. Delete otherwise. -->
<!-- Before/after numbers, same machine, exact command. -->

| | Before | After |
|---|---|---|
| fps / dropped frames | | |
| bytes per frame | | |
| CPU (one core %) | | |

## Checklist

- [ ] One concern in this PR (no drive-by refactors)
- [ ] Tests added or updated for the new behavior
- [ ] Docs updated if CLI, protocol support or budgets changed
- [ ] `CHANGELOG.md` updated under `## [Unreleased]` for user-visible changes
- [ ] Commits signed off (DCO, `git commit -s`)
- [ ] No secrets, no unrelated files, no new dependency without prior agreement
