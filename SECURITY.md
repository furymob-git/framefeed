# Security policy

## Supported versions

framefeed has not cut a release yet (design phase, M0). Security fixes will target the
latest released minor version once `0.1.0` ships; pre-release tags are unsupported.

## Reporting a vulnerability

Please **do not open a public issue**.

- Preferred: GitHub private vulnerability reporting on this repository
  ("Security" tab → "Report a vulnerability").
- Or email: eliott.pereira-rodrigues-gameiro@epita.fr

Include: affected version/commit, platform and terminal, a minimal reproduction (a
command plus a source file if needed), and impact. We will acknowledge within 7 days
and aim to ship a fix or mitigation within 30 days for confirmed issues. Credit is
given in the changelog unless you ask otherwise. No bug bounty is offered.

## Threat model (what we care about)

framefeed is a client that parses untrusted media and network manifests, then writes
bytes to a terminal. The interesting attack surfaces:

1. **Terminal escape injection.** Titles, metadata, URLs, and error messages are printed
   to a terminal that interprets escape sequences. Any untrusted string displayed in the
   status line or logs must be sanitized (control characters stripped/escaped).
2. **Media decoder.** The `ffmpeg` sidecar parses untrusted containers. We pass an
   explicit protocol whitelist, never interpolate arguments through a shell, and treat
   decoder crashes as recoverable.
3. **Network sources (M3/M5).** Redirects, playlist URLs, and segment URLs can point at
   internal services. `serve` needs an SSRF guard (scheme allowlist, DNS resolution with
   private-range rejection) and hard resource limits.
4. **Resource exhaustion.** Playlists can declare enormous variants, endless segment
   lists, or huge frames. All sizes are capped; allocations are checked.
5. **Supply chain.** Locked dependencies in CI (`--locked`), `cargo-deny` for licenses
   and advisories, Dependabot for updates, no build scripts from unknown crates without
   review.

## Out of scope

- Vulnerabilities in the terminal emulator itself (report upstream).
- Vulnerabilities in `ffmpeg` reachable only with a specially crafted file (report
  upstream, and tell us if our argument handling makes it worse).
- DRM circumvention, or playback of content you are not authorized to access.
