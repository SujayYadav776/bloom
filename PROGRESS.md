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

## Feature: New notch panel — Wallpaper changer

> Replaced the short-lived System Monitor panel (reverted) at the user's request.

- [x] Register `wallpaper` as a cycleable `NotchPanel` + `BloomMode`
- [x] Rust `src-tauri/src/wallpaper.rs`: `get_wallpaper_thumbnails` (scans Windows `Web\Wallpapers`/`Web\Screen`, user + public Pictures, optional `bloom-wallpaper-folder`), `set_wallpaper` (SystemParametersInfoW), `get_current_wallpaper`
- [x] Base64 JPEG thumbnails rendered under the existing CSP (no asset-protocol/CSP/capability changes; no new Cargo deps — `image`/`base64` already present)
- [x] Notch thumbnail grid with hover-enlarge + active ring; click sets the wallpaper
- [x] `tsc && vite build` typecheck passes

## Build & ship

- [x] Dependencies installed faithfully via `bun install --frozen-lockfile` (matches `Cargo.lock`)
- [x] Full Windows installer built: `tauri build --bundles nsis` → `bloom_3.10.1_x64-setup.exe`
- [x] Local-only override `tauri.build-local.json` disables updater signing for local builds (kept out of git)
- [ ] Ongoing: keep upstream synced via merge commits

## Planned ideas (not started)

- [ ] Additional notch panels (e.g. unit converter, world clock)
- [ ] Dock-side custom widget
- [ ] Per-monitor theme memory
