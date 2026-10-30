# Sundry

Native macOS packing-list app. You enter a destination and month; it generates a grounded, season-aware checklist on-device using Apple Intelligence, then lets you check items off as you pack.

Free to run. No API keys. Nothing leaves the Mac except a one-time climate lookup to [Open-Meteo](https://open-meteo.com).

## Requirements

- macOS 26 (Tahoe) or later
- Apple Silicon
- Apple Intelligence enabled
- Xcode 26 (full Xcode, not just Command Line Tools) for `@Generable` macros
- Rust (`rustup` or Homebrew)

## Getting started

1. Make sure you are on **macOS 26 or later** on an Apple Silicon Mac, and turn on **Apple Intelligence** in System Settings → Apple Intelligence & Siri (the model downloads in the background the first time).
2. Install Sundry (open the DMG, drag **Sundry → Applications**, then **right-click → Open** the first time because the build is ad-hoc signed).
3. Click **File → New Trip**. Enter a place and month, pick nights and purposes, and list who is packing. The first traveller defaults to your macOS name; add, rename, or remove travellers in the sheet. Your choices become the defaults for the next trip.
4. Click **Create and Generate**. Sundry looks up climate normals for that month, then builds the list on-device, one category at a time.
5. Pack items into each traveller's column. Rename or add travellers for an existing trip from the toolbar (right-click a name to remove it).
6. To share a trip with someone else using Sundry, use the AirDrop button; keep Sundry open on both Macs to sync.

## Layout

```
sundry-core/     Rust domain, Automerge documents, generation pipeline
sundry-ffi/      UniFFI surface + generated Swift bindings
sundry-mac/      SwiftUI app
docs/            design spec and implementation plan
```

## Build the Mac app

```bash
# 1. Rust core (also runs as an Xcode build phase)
cargo test
cargo build -p sundry-ffi --release --target aarch64-apple-darwin

# 2. Open in Xcode and run the Sundry scheme
open sundry-mac/Sundry.xcodeproj
```

The first launch asks you to enable Apple Intelligence if it is off. Create a trip with a place and month; generation fills the checklist one category at a time.

## Package a DMG (share with another Mac)

```bash
./scripts/package-dmg.sh
```

Produces `dist/Sundry-0.2.1.dmg`. AirDrop or send it; the recipient opens the DMG, drags **Sundry → Applications**, then **right-click → Open** the first time (free ad-hoc signing — no Apple Developer fee). A paid Developer ID would remove that Gatekeeper step.

## Regenerating UniFFI bindings

```bash
./sundry-ffi/build-xcframework.sh
```

Copy updated `sundry_ffi.swift` and `sundry_ffiFFI.h` into `sundry-mac/Vendor/` if the FFI surface changed.

## License

[MIT](LICENSE)
