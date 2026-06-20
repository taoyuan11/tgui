# Security Policy

## Supported versions

Security fixes are accepted for the current `master` branch and the latest
published 0.x release when a patch release is practical. Older 0.x releases are
best-effort until the project reaches 1.0.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Email the
maintainer listed in `Cargo.toml` with:

- affected version or commit;
- platform and feature set;
- reproduction steps or proof of concept;
- impact and any known mitigations.

Expected response window:

- acknowledgement within 7 days;
- initial triage target within 14 days;
- coordinated disclosure date agreed after impact is understood.

## Scope

Security-sensitive areas include media URL loading, SVG external resources,
FFmpeg audio/video decoding, notification payload escaping, platform FFI,
dialog/clipboard integration, and any future packaging/signing scripts.

## Dependency advisories

`cargo-deny` runs in CI. Advisory ignores must include a rationale, affected
surface, mitigation, and review date in `deny.toml` or the PR description.

## Current defaults

Remote image loading only allows `http` and `https`, limits redirects, uses
timeouts, and caps response bodies. Applications that load untrusted paths or
URLs should still apply their own sandbox roots, host allowlists, and content
policy before constructing `MediaSource`.
