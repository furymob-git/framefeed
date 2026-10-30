# Contributing to framefeed

Thanks for helping. This project is design-phase (M0), so the highest-value
contributions right now are **protocol research, terminal conformance reports, and
benchmark methodology** — plus the M1 implementation slices in
[docs/ROADMAP.md](docs/ROADMAP.md).

By participating you agree to our [Code of Conduct](CODE_OF_CONDUCT.md).

## Ground rules

- **Everything lands through a pull request.** `main` is protected: no direct pushes,
  no force-pushes, linear history, required checks.
- **One concern per PR.** A refactor and a behavior change in the same PR get separate
  reviews and will be asked to split.
- **Open an issue first** for anything larger than a bug fix: new backend, new
  dependency, pipeline/threading change, CLI surface change. Those need agreement
  before code.
- **No unrequested scope.** Bug fixes do not come with drive-by refactors.
- **Performance claims need numbers.** Before/after, same machine, command included.

## Development setup

Requirements: Rust stable (the repo pins it in `rust-toolchain.toml`), `git`, and
`ffmpeg` on `PATH` (used as a decoder process from M1 on).

```console
# Rust: https://rustup.rs — then:
rustup show                      # picks up rust-toolchain.toml
cargo build
cargo test

# ffmpeg
winget install Gyan.FFmpeg       # Windows
brew install ffmpeg              # macOS
sudo apt install ffmpeg          # Debian/Ubuntu
```

Terminal for manual testing:

- **Windows:** WezTerm with `enable_kitty_graphics = true` (Kitty backend) or Windows
  Terminal 1.22+ (Sixel). Kitty itself does not run on Windows.
- **Linux:** kitty (Kitty backend), foot (Sixel), Alacritty (ANSI fallback).
- **macOS:** kitty, WezTerm, or iTerm2.

## Checks

Run these before pushing; CI runs exactly the same commands.

| Command | What it guards |
|---|---|
| `cargo fmt --all --check` | formatting |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | lints |
| `cargo test --workspace --all-features --locked` | unit + golden + integration |
| `cargo doc --workspace --no-deps` | rustdoc links (warnings are errors in CI) |
| `cargo deny check` | licenses, advisories, banned crates |
| `cargo bench` | performance (informational locally) |

End-to-end playback tests are opt-in because they need `ffmpeg` and a graphics-capable
terminal:

```console
FRAMEFEED_E2E=1 cargo test --test e2e -- --nocapture
```

## Branching and commits

- Branches: `feat/<slug>`, `fix/<slug>`, `docs/<slug>`, `perf/<slug>`, `chore/<slug>`.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/):
  `feat(proto): add kitty placement reuse`, `fix(pacer): avoid drift on seek`.
  Scopes are crate or subsystem names: `cli`, `core`, `proto`, `decode`, `source`,
  `server`, `docs`, `ci`.
- Sign off your commits (`git commit -s`): this project uses the
  [DCO](https://developercertificate.org/) instead of a CLA.
- Rebase on `main`; do not merge `main` into your branch.

## Pull requests

The template asks for: what changed, why, how it was tested, and the performance impact
(if any). A PR is ready when:

1. It has a linked issue (or is a trivial fix with an explanatory description).
2. CI is green on Linux, macOS and Windows.
3. New behavior has tests (unit, golden bytes for encoders, or an opt-in e2e).
4. Docs are updated when the CLI, protocol support, or budgets change.
5. `CHANGELOG.md` has an entry under `## [Unreleased]` for user-visible changes.
6. Perf-sensitive changes include measured before/after numbers.

Review is conversational; expect questions about budgets and terminal coverage. Merge
is **squash-only** with the PR title as the commit message (it must be Conventional).

## Where to change what

| Area | Path |
|---|---|
| CLI surface, command wiring | `crates/framefeed-cli` |
| Pipeline, frame types, pacer | `crates/framefeed-core` (M1) |
| Display protocols, capability probe | `crates/framefeed-proto` (M1) |
| Decoders | `crates/framefeed-decode` (M1) |
| Files/HTTP/HLS, quality selection | `crates/framefeed-source` (M3) |
| `serve` endpoint | `crates/framefeed-server` (M5) |
| Design decisions | `docs/`, ADRs for anything expensive to reverse |

## Labels

`good first issue`, `help wanted`, `backend:kitty`, `backend:sixel`, `backend:ansi`,
`area:decode`, `area:streaming`, `area:perf`, `needs-rfc`, `needs-repro`.

## Releases

`main` is always releasable. Releases are cut from a tag on `main`:
`vMAJOR.MINOR.PATCH` (SemVer), changelog finalized from `Unreleased`, binaries built by
CI (`cargo-dist`, M6). Pre-1.0 breaking changes are allowed in `0.MINOR` bumps and must
be called out in the changelog.

## Repository settings (maintainer checklist)

At repo creation, before announcing:

- [ ] Enable branch protection on `main`: require PR, require the CI checks, require
  linear history, block force-push and deletion.
- [ ] Enable Dependabot alerts, secret scanning, private vulnerability reporting.
- [ ] Create labels listed above and milestones M1–M6.
- [ ] Set the repository description, topics (`terminal`, `video`, `kitty`,
  `sixel`, `hls`, `rust`), and social preview.
- [ ] Update the placeholder owner in `README.md` badges and `Cargo.toml` metadata.
