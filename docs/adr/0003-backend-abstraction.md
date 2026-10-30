# 0003 — Byte-oriented display backend abstraction

- Status: Accepted
- Date: 2026-10-30

## Context

Terminal graphics protocols differ wildly: Kitty can reuse image ids, animate, and read
from shared memory; Sixel repaints a palette-indexed raster; iTerm2 uploads a full
image per frame; ANSI writes text cells. We need one pipeline above them, and we want
to test encoders without a terminal.

We could depend on `notcurses` (C library with Kitty + Sixel support) instead of
writing encoders.

## Decision

Define a `Backend` trait whose core operation is
`encode(&Raster, &mut Vec<u8>) -> Result<()>`: pure, byte-oriented, no I/O, no clock,
no terminal access. Terminal lifecycle (raw mode, ids, shared memory, restore) is
handled by `prepare`/`restore` and a guard type.

Do not depend on notcurses.

## Consequences

- Golden tests can assert exact escape bytes per protocol, in CI, with no terminal.
- Backends declare a `Budget` (max raster, max fps, expected bytes/frame) which the
  pipeline uses to pick defaults; performance policy stays out of the encoders.
- We control compression policy, chunking, cursor movement and kitty animation, which
  is exactly where the performance wins live.
- Cost: we own the protocol details and their conformance bugs. Mitigation: a
  `doctor` conformance check per terminal, and tests against recorded terminal
  responses.
- Cost: no notcurses ecosystem features (widgets, input handling) — out of scope
  anyway; we do not need a TUI toolkit.

## Alternatives considered

- **notcurses**: production-grade Kitty/Sixel, but a C dependency, its own rendering
  model, and little control over our byte budget and animation strategy. It would also
  make Windows builds and static release binaries harder.
- **chafa as a subprocess**: excellent image-to-ANSI quality, but no video pacing, no
  kitty image reuse, and a process per frame.
- **One backend only (Kitty)**: simpler, but abandons Windows Terminal, iTerm2 and the
  degrade-never-fail principle.
