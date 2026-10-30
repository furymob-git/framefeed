# Architecture decision records

Short, immutable records of decisions that are expensive to reverse. Format:
context → decision → consequences. Superseding an ADR means adding a new one that
references it; do not rewrite history.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-rust.md) | Implementation language: Rust | Accepted |
| [0002](0002-ffmpeg-sidecar.md) | Decoder: ffmpeg sidecar first, libav later | Accepted |
| [0003](0003-backend-abstraction.md) | Byte-oriented display backend abstraction | Accepted |
| [0004](0004-quality-model.md) | Quality as render budget + source variant | Accepted |

Template:

```markdown
# NNNN — Title

- Status: Proposed | Accepted | Superseded by NNNN
- Date: YYYY-MM-DD

## Context
## Decision
## Consequences
## Alternatives considered
```
