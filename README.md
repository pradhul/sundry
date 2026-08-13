# Sundry

Native macOS packing-list app. You enter a destination and month; it generates a grounded, season-aware checklist on-device using Apple Intelligence, then lets you check items off as you pack.

Free to run. No API keys. Nothing leaves the Mac except a one-time climate lookup to [Open-Meteo](https://open-meteo.com).

## Requirements

- macOS 26 (Tahoe) or later
- Apple Silicon
- Apple Intelligence enabled
- Xcode 26 (full Xcode, not just Command Line Tools) for `@Generable` macros
- Rust (`rustup` or Homebrew)

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

The first launch asks you to enable Apple Intelligence if it is off. Create a trip with city + ISO country code (e.g. Kyoto / JP) and a month; generation fills the checklist one category at a time.

## Regenerating UniFFI bindings

```bash
./sundry-ffi/build-xcframework.sh
```

Copy updated `sundry_ffi.swift` and `sundry_ffiFFI.h` into `sundry-mac/Vendor/` if the FFI surface changed.
