# GLP Atlas

A GLP-1 dose and progress tracker for Android, written in Rust with [Dioxus](https://dioxuslabs.com).

**This is early.** What exists today is the navigation shell with sample numbers wired into
it. There's no database yet, so nothing you enter sticks around. Local storage is the next
piece of work. [ROADMAP.md](ROADMAP.md) lays out the plan, including the things this app
will deliberately never do.

## The idea

Most health trackers are a stack of white cards, and every screen change is a hard cut to
another stack of white cards. I wanted this one to feel like a place instead, so the whole
app lives inside a single twilight star field.

The three top-level pages (Doses, Progress, Profile) sit side by side in that field, and
anything you open from one of them sits deeper in. Navigating moves the camera rather than
replacing the screen. Each of the three keeps its own stack, so backing out of a sub-page
under Progress leaves your place under Doses alone. You can swipe between the three, tap the
tab bar, or use the Android back gesture.

## Trying it

The quickest way to see it is in a browser:

```sh
make web
```

The UI doesn't touch any platform APIs, so Chrome renders what the phone renders. Running it
on a real device needs the Android SDK and NDK first. [CONTRIBUTING.md](CONTRIBUTING.md)
covers that and the rest of the build targets.

## Licence

Copyright (C) 2026 DipityCo. [AGPL-3.0-or-later](LICENSE), and that isn't going to
change. The app itself stays free and open; the plan is to fund it with a hosted service
for people who'd rather not run one themselves. ROADMAP.md spells out which features that
promise covers.
