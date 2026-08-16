# Contributing

## What you'll need

- dioxus-cli 0.7.x
- JDK 17 or newer
- Android SDK, platform 36
- Android NDK
- shellcheck

## Working on it

`make` lists every target. Nothing in the UI touches a platform API, so `make web` renders in
a browser what the phone renders, with no Android toolchain involved. `make hotpatch` carries
Rust alone; Kotlin and manifest changes need the full `make install`.

Android build settings live in two places. `app/Dioxus.toml` holds everything `dx` reads, for
both `dx serve` and packaged builds; `scripts/build-android.sh` covers what dx exposes no
hook for, such as the launcher label and icon. A setting that appears to be ignored is set in
the other file.

## Before you open a PR

`make ci` has to pass. Clippy runs pedantic with `-D warnings`, over the lint set in
`app/Cargo.toml`. GitHub Actions runs the same targets on every pull request, so a green
local run is a green CI run. The APK is not built per pull request; it is built on merge to
main and on demand.

`make test` covers the navigation state machine and the star field's generation. Nothing
renders a component, so verify UI changes by running the app, and say what you ran and on
what.

Write commit subjects as [conventional commits](https://www.conventionalcommits.org). The
changelog is generated from them.

Read [ROADMAP.md](ROADMAP.md) before building a feature. Some things are out of scope by
design, and a few are permanently excluded.

## Releasing

release-plz keeps a pull request open that carries the version bump and the CHANGELOG.md
entries for everything merged since the last release. Merging it is what releases: the tag,
the draft GitHub release and the release APK attached to it all follow from that. Publishing
the draft is the only manual step.

## Licence

Contributions are made under the project's licence, AGPL-3.0-or-later.
