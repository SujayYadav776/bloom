# Bloom (Sujay's fork) — Progress Tracker

This repository is a **fork of [SehajveerSingh2005/bloom](https://github.com/SehajveerSingh2005/bloom)**,
kept under the original **GPL-3.0-only** license with upstream attribution preserved.
This file tracks what has been added on top of upstream.

Legend: `[x]` done · `[~]` in progress · `[ ]` planned

## Baseline

- [x] Fork of upstream `main` synced (up to `2509107`, v3.10.1+ PRs)
- [x] Upstream remotes wired (`origin` = SujayYadav776/bloom, `upstream` = SehajveerSingh2005/bloom)
- [x] GPL-3.0 license and upstream attribution retained

## Feature: New notch panel — Scratchpad quick notes

- [x] Register `scratchpad` as a cycleable `NotchPanel` + `BloomMode`
- [x] Add panel state, text persistence, and clear action
- [x] Panel body, sizing (width/height), and styling
- [x] `tsc && vite build` typecheck passes

## Feature: Theme & appearance — accent preset swatches

- [x] One-click curated accent presets in the Appearance settings tab
- [x] Presets switch theme to Custom and apply color + derived saturation/brightness
- [x] `tsc && vite build` typecheck passes

## Build & ship

- [~] Full Windows installer via `tauri build`
- [ ] Ongoing: keep upstream synced via merge commits

## Planned ideas (not started)

- [ ] Additional notch panels (e.g. system monitor, unit converter)
- [ ] Dock-side custom widget
- [ ] Per-monitor theme memory
