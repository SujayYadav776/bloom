# Bloom (Sujay's fork) — Progress Tracker

This repository is a **fork of [SehajveerSingh2005/bloom](https://github.com/SehajveerSingh2005/bloom)**,
kept under the original **GPL-3.0-only** license with upstream attribution preserved.
This file tracks what has been added on top of upstream.

Legend: `[x]` done · `[~]` in progress · `[ ]` planned

## Baseline

- [x] Fork of upstream `main` synced (up to `2509107`, v3.10.1+ PRs)
- [x] Upstream remotes wired (`origin` = SujayYadav776/bloom, `upstream` = SehajveerSingh2005/bloom)
- [x] GPL-3.0 license and upstream attribution retained

## Feature: Scratchpad quick notes — REMOVED

> Removed at the user's request (superseded by the Wallpaper panel).

- [x] ~~Scratchpad notch panel~~ removed from `App.tsx` / `App.css`

## Feature: Theme & appearance — accent preset swatches

- [x] One-click curated accent presets in the Appearance settings tab
- [x] Presets switch theme to Custom and apply color + derived saturation/brightness
- [x] `tsc && vite build` typecheck passes

## Feature: New notch panel — Wallpaper changer

> Replaced the short-lived System Monitor panel (reverted) at the user's request.

- [x] Register `wallpaper` as a cycleable `NotchPanel` + `BloomMode`
- [x] Rust `src-tauri/src/wallpaper.rs`: `set_wallpaper` (SystemParametersInfoW), `get_current_wallpaper`, `get_wallpaper_thumbnails`, `get_wallpaper_folder`
- [x] Source is a single dedicated user folder — default `%USERPROFILE%\Pictures\Bloom Wallpapers`, created on demand, overridable via the `bloom-wallpaper-folder` setting (no longer scans the Windows default locations); its path is shown in the panel header so you know where to drop your own images
- [x] Base64 JPEG thumbnails rendered under the existing CSP (no asset-protocol/CSP/capability changes; no new Cargo deps — `image`/`base64` already present)
- [x] Reworked to a single horizontal scrolling thumbnail strip with explicit prev/next page buttons; wheel over the strip scrolls horizontally (stopPropagation so it doesn't cycle panels); count badge; larger 16:9 tiles; stronger active ring
- [x] Header actions: "Choose folder" (native directory picker → saves `bloom-wallpaper-folder` and rescans) and "Refresh" (rescans the folder without restarting the app)
- [x] `tsc && vite build` typecheck passes

## Fix: Shelf drag-and-drop reliability

> Root cause of "can't drag a file onto the notch": not a code bug and not an
> elevation/UIPI problem. The OLE drop target is registered on the notch's
> WebView2 child windows, and those get torn down and rebuilt over time
> (sleep/wake, DPI change, webview reload). `setup_shelf_drop` only re-registered
> at startup + 1.5s, so a long-running notch silently stopped receiving drops.
> A fresh launch re-registered on the live child windows and worked flawlessly.

- [x] Instrumented the drop path (`registered targets=N`, `DragEnter`, `Drop`) in `diagnostics.log`; a real drag confirmed the full flow (targets=6 → DragEnter files=1 → Drop files=1)
- [x] Added a freshness watchdog in `shelf.rs`: a background poll (every 4s, marshalled onto the main thread) re-registers the drop target whenever the live child-window set differs from the set we registered on
- [x] Watchdog holds off while a drag is in flight (`SHELF_DRAG_ACTIVE`) so an active drop is never interrupted by a revoke/re-register cycle
- [x] `cargo check` passes

## Build & ship

- [x] Dependencies installed faithfully via `bun install --frozen-lockfile` (matches `Cargo.lock`)
- [x] Full Windows installer built: `tauri build --bundles nsis` → `bloom_3.10.1_x64-setup.exe`
- [x] Local-only override `tauri.build-local.json` disables updater signing for local builds (kept out of git)
- [ ] Ongoing: keep upstream synced via merge commits

## Planned ideas (not started)

- [ ] Additional notch panels (e.g. unit converter, world clock)
- [ ] Dock-side custom widget
- [ ] Per-monitor theme memory
