# Sundry — Design

**Date:** 2026-08-13
**Status:** Approved, ready for implementation planning

## Overview

Sundry is a native macOS app that produces a destination-aware, seasonally-aware packing checklist for a trip, and lets you check items off as you pack. You enter where you're going and roughly when; it generates an itemised list covering the things that are easy to forget precisely because they're specific to that destination — the Type F plug adapter for Germany, thermals for Reykjavik in February, a rain shell for Kyoto in November.

It is built for two users (the author and his wife) packing for shared trips. It is not a product for strangers, which removes App Store constraints and licensing concerns.

**Hard constraint: the app must be free to run, including the AI.** No subscriptions, no per-token costs, no paid API tiers. This is a requirement, not a preference, and it shapes the design more than anything else below.

## Problem

Generic packing lists are useless because everything interesting about packing is contextual. A checklist app can tell you to bring socks; it can't tell you that your devices won't fit the sockets where you're going. The knowledge that matters is destination- and season-specific, and re-deriving it by hand for every trip is exactly the kind of tedious research that gets skipped, which is how you end up buying an adapter at airport prices.

## Non-goals

- Not a trip planner, itinerary builder, or booking tool.
- Not a shared social or collaborative product beyond two known users.
- Not an authority on visa or entry requirements. See "Travel documents" below.
- Not cross-platform in v1. The Rust core is portable by construction, but only a macOS frontend is planned.

## Decisions and rationale

These were settled during brainstorming and are recorded here so the reasoning survives.

**Packing judgement comes from an LLM; facts come from data.** The rejected alternative was a hand-authored rules engine, which would have meant a permanent content-maintenance commitment for a two-person app. The split adopted instead is that hard facts (sockets, voltage, climate) are looked up, never generated, while the judgement of what those facts imply for a bag is the model's job. Its output is validated rather than trusted.

**Inference runs on-device via Apple's Foundation Models framework.** This is the only option that satisfies the free constraint with no asterisks: no API key, no account, no cost, no rate limit, and nothing leaves the machine. Free cloud tiers were rejected on privacy grounds — they generally reserve the right to train on submitted data, and this app holds passport and travel document details. A local Ollama model would also be free and private but requires a multi-gigabyte download and manual setup on each machine.

**The on-device model is never asked to recall facts.** Apple is explicit that the ~3B model is "not designed to be a chatbot for general world knowledge" and advises against relying on it for factual recall. Destination knowledge is exactly that, so on a naive design this model would be the wrong tool. The design compensates by grounding aggressively: every fact the model needs is supplied in the prompt, reducing its job to structured reasoning over given context — which is what it genuinely is good at. If grounding is ever weakened, output quality collapses. This is the single most important constraint in the system.

**The view layer is SwiftUI, not Rust.** SwiftUI cannot be driven from Rust; it depends on Swift-only language features and exposes no C or Objective-C ABI. AppKit *can* be driven from Rust via `objc2-app-kit`, but that means hand-written delegate plumbing, pervasive `unsafe`, and an AppKit-era appearance. `cacao`, the safe wrapper that would have made this pleasant, is beta with stalled maintenance. Webview frameworks (Tauri, Dioxus) were rejected outright: the requirement was a Mac app, not a web app in a window. GPUI draws its own controls and so looks like Zed rather than like a Mac app.

**The Rust core must own real work, or the FFI boundary isn't worth it.** With an LLM-first design and a Swift view layer, a naive split would leave Rust with one HTTP call and some persistence. The core therefore owns the domain model, the document, the generation pipeline, validation, and merge semantics.

**Storage is an Automerge CRDT document from day one, even though v1 has no sync.** Sync across two Macs is the eventual core need, and two people checking items off — sometimes offline in a hotel with no roaming data — is a genuine concurrent-edit problem. A CRDT makes every transport safe and additive: CloudKit when online, Bonjour peer-to-peer on the same Wi-Fi, or AirDropping the trip file. With last-writer-wins storage, AirDrop would mean one person's copy clobbers the other's. Building on Automerge now costs little; retrofitting it later means redoing the data layer.

**Network is only needed at trip creation, and only for climate data.** Inference itself is fully offline. Climate normals are fetched once when a trip is created, which happens at home on Wi-Fi. Everything on the road — checking off, regenerating, syncing — needs no network at all.

**Because inference is free and local, regeneration is cheap.** There is no cost pressure discouraging the user from regenerating a list, which makes the non-destructive merge rule below more valuable rather than less.

## Architecture

Three layers, with a hard rule: Swift contains no business logic, and Rust knows nothing about views. There is exactly one carefully-bounded exception, described below.

```
sundry/
  sundry-core/    Rust library — domain model, Automerge document, generation
                  pipeline, country facts, persistence. No FFI, no Swift.
  sundry-ffi/     Rust crate — the deliberate #[uniffi::export] surface.
                  Builds to an xcframework consumed by Swift Package Manager.
  sundry-mac/     SwiftUI application — views, app lifecycle, and the
                  Foundation Models inference backend.
  docs/           Specs and design documents.
```

`sundry-core` is deliberately separate from `sundry-ffi` so the core stays a normal Rust library, unit-testable with `cargo test` and no Swift or Xcode involvement. The FFI crate keeps the bridge surface small and intentional rather than letting it sprawl.

The bridge is **UniFFI** (Mozilla), which generates idiomatic Swift including `async/await` mapping for async Rust functions. Version pinned at implementation time via `cargo add`. The core is exposed as a single UniFFI object, mapped to a Swift actor so generation never blocks the main thread.

### The inference backend and the one exception to the layering rule

Foundation Models is a Swift-only framework, so the inference call itself cannot live in Rust. This is the sole place where Swift does something other than draw views, and it is worth being precise about the boundary.

Rust declares an `LlmBackend` trait exposed as a **UniFFI callback interface**. Swift implements it as a thin adapter over `LanguageModelSession`. Rust still owns the entire pipeline: it assembles the grounded prompt, decides how the work is split, validates whatever comes back, and performs the merge. Swift's implementation contains no logic beyond "run this prompt through the model and hand back the result." The inference backend is a transport, in the same way the sync transports are.

This has a valuable side effect: because the backend is an injected trait, `sundry-core` can be tested end to end against a fake backend that replays recorded responses, with no Apple frameworks and no model involved.

**Both outbound I/O paths are Swift-implemented transports, and the Rust core does no networking.** `LanguageModelSession` is `async`, so the callback interface must be async too, which means the Rust pipeline is async. Once that's true, the climate lookup is best implemented the same way — a second async callback interface backed by `URLSession` — rather than pulling an HTTP client, TLS stack, and async runtime into Rust for one request. The core is left with pure logic plus file persistence, no HTTP dependencies, and no async runtime beyond what UniFFI provides. Rust still owns orchestration, parsing, validation, and merging; Swift only moves bytes.

**Schema duplication is accepted deliberately.** Foundation Models' guided generation constrains output at the token level via the `@Generable` macro on a Swift type, which guarantees the result parses. That guarantee is worth more than avoiding a duplicated type, so the response shape is declared twice — as a `@Generable` struct in Swift and as the authoritative type in Rust. Swift's copy exists only to drive constrained decoding; Rust remains the source of truth and still performs all semantic validation, since constrained decoding guarantees well-formed output, not sensible output.

**Platform:** macOS 26.0 (Tahoe) minimum, on Apple Silicon, with Apple Intelligence enabled. This is dictated by Foundation Models rather than chosen. Tahoe runs on every Apple Silicon Mac including the original M1; Intel Macs cannot run this app at all, and both target machines are Apple Silicon. The app is sandboxed from the start with the outgoing-network entitlement, because retrofitting sandboxing once CloudKit is added is painful.

**Persistence:** one Automerge document per trip at `Application Support/Sundry/trips/<uuid>.automerge`, plus an `index.json` listing trips for fast sidebar population without opening every document. Per-trip documents keep sync payloads small and make a trip a self-contained thing that can be handed to someone — which is what makes the AirDrop transport possible later.

## Data model

**Trip** — id, title, destinations, travel window (month or explicit date range), nights, travellers, purpose tags (business, leisure, hiking, beach, city), timestamps.

**Destination** — ISO 3166-1 alpha-2 country code, city, optional arrival and departure dates. Destinations are city-level, not country-level: Reykjavik and Tenerife are both "Europe" and are not remotely the same packing problem.

**Category** — a fixed taxonomy, not model-generated: Documents, Money, Electronics, Clothing, Footwear, Toiletries, Health & Medication, Bags & Organisation, Comfort & Entertainment, Activity-Specific, Miscellaneous. Fixed because a model left to invent categories will produce fifteen overlapping ones across two trips and the app will feel incoherent.

**Item** — id, name, quantity, category, owner, state, source, rationale, optional note, essential flag.

Two fields deserve explanation. `source` records whether an item came from the model, from the user by hand, or (later) from learned defaults. `rationale` is the one-line reason the item was suggested — "Germany uses Type F sockets and your devices are Type D". The rationale is what makes the app feel like it knows something rather than listing nouns, and it's surfaced in the inspector.

**Item state** is three-valued — `todo`, `packed`, `notNeeded` — rather than a boolean. "I deliberately don't need this" is different information from "I haven't got to it yet". Keeping them distinct is what makes the post-trip learning loop possible later without reworking the model.

**Owner** is present on the model in v1 (shared, or a specific traveller) but not surfaced in the v1 interface. The field costs nothing now and avoids inconsistency once per-person ownership ships alongside sync.

## Generation pipeline

Orchestrated entirely within `sundry-core`. Two steps reach outside it — the climate lookup and the inference call — and both are injected traits rather than direct calls, which is what keeps the pipeline testable.

1. **Assemble trip context** from the trip record.
2. **Ground it** with a bundled country facts table: plug types, mains voltage and frequency, currency code, driving side, tap water potability. Roughly 200 rows keyed by ISO 3166-1 alpha-2, authored once as a checked-in JSON file under `sundry-core/data/` from public reference sources, compiled into the binary via `include_str!` and parsed into a lookup map at startup. These are stable public facts that effectively never change, so there is no update mechanism and none is needed. Supplying them rather than asking for them eliminates the most embarrassing class of error — the model should be reasoning about what 100V means for your hair dryer, not guessing whether Japan is 100V.
3. **Ground it further with climate normals** from **Open-Meteo**, which is free, requires no API key, and needs no account. The destination city is geocoded through Open-Meteo's free geocoding endpoint, then monthly climate normals — average high and low, precipitation, daylight hours — are fetched for the travel month. The result is cached onto the trip, so regenerating later needs no network. Like the inference backend, the climate provider sits behind a trait, so tests never touch the network. This step is what makes the on-device model viable: it converts "what's the weather like in Kyoto in November" from a recall question the model would fail into a fact supplied in the prompt.
4. **Generate one category at a time**, a separate inference call per category. See below.
5. **Parse and validate** into typed structs: known category, non-empty name, sane quantity, deduplicated on normalised name. Constrained decoding guarantees the output is well-formed, not that it is sensible, so semantic validation still happens in Rust.
6. **Merge into the Automerge document** additively.

### Why generation is split per category

Asking for a complete sixty-item list in a single call is the obvious approach and the wrong one here. The on-device model has a small context window, and Apple's own guidance is to break work into smaller pieces to get good results. A single large request risks truncation and topic drift, and one bad response ruins the whole list.

One call per category — eleven small, focused prompts, each carrying the same grounded facts but asking a narrow question — is better on every axis. Each prompt stays well inside the context limit, adherence improves because the task is narrow, a failure is isolated to one category and retriable on its own, and categories can be populated progressively in the interface. That last point means we get streaming-like feedback for free, without a streaming API and without fighting constrained decoding.

### Merge semantics

This is the load-bearing rule: **regeneration is additive and never mutates existing state.**

Regenerating a list after packing half your bag must leave packed items packed, leave `notNeeded` items as they are, and never delete items you added by hand just because the model didn't think of them this time. Matching is by normalised name within category. Without this guarantee regeneration would be destructive, and you would quickly learn never to press the button.

### Travel documents

Documents appear as a normal `Documents` category in the generated list in v1. Sundry never asserts a visa or entry rule as fact: those rules depend on your specific passport, layovers, and purpose of travel, they change without notice, and getting one wrong means missing a flight. Official-source links and the passport expiry watch are on the roadmap; asserted rules are permanently out of scope.

## Interface

`NavigationSplitView`: trips in the sidebar grouped into upcoming and past, the checklist in the detail pane, an inspector for the selected item's rationale and notes. New trips are created in a sheet — destination, travel window, nights, travellers, purpose tags.

The checklist is a sectioned `List` grouped by category with progress in the toolbar. It is keyboard-first because packing is repetitive: arrow keys to navigate, space to toggle packed, and `.searchable` for finding one specific thing. Menu bar commands and `⌘N` come free from being a real Mac app, which is the entire point of choosing Swift for the view layer.

Generation fills the list in progressively, one category at a time, because that is how the pipeline runs anyway — each category appears as its inference call completes, with the pending ones shown as placeholders. This gives the feel of streaming without any streaming machinery. Token-level streaming within a category is out of scope: it fights constrained decoding and buys very little once categories already arrive incrementally.

## Failure handling

The organising principle: **creating a trip and generating its list are separate operations**, so a failed generation never costs you the trip. A trip with no list is a valid state with a retry action, not an error.

- **Apple Intelligence unavailable** — the framework distinguishes "not enabled", "model still downloading", and "device ineligible". These are three different situations needing three different messages, and the first two are onboarding states with a clear next action, not errors. Ineligibility cannot occur on the target hardware but is handled rather than crashed on.
- **Climate lookup fails** — generation proceeds using country facts alone, and the trip is explicitly marked as lacking climate grounding with an offer to re-ground later. It must never silently produce a weaker list without saying so, because a list that looks authoritative but wasn't grounded is worse than an obviously incomplete one.
- **A category fails** — retried on its own. Because generation is split per category, one failure costs one category rather than the whole list.
- **Output is well-formed but nonsensical** — constrained decoding guarantees structure, not sense. Semantic validation failure triggers one retry of that category, then surfaces a clear message.
- **Nothing is partially applied** — the full item set for a category is built and validated before it touches the document.

## Testing

Because the logic lives in Rust, most of the app is testable via `cargo test` with no app launch and no network.

- **Merge semantics** — regeneration preserves item state. This is the test that matters most; if it silently broke, the app would quietly become untrustworthy.
- **Output validation** — recorded model responses as golden files, including malformed and nonsensical ones.
- **Prompt assembly** — snapshot tests asserting that grounded facts actually reach the prompt. Since grounding is what makes a 3B model viable, a regression that quietly drops the climate block would degrade quality with no visible error, which makes it exactly the kind of bug worth a test.
- **Country facts lookup** and **Automerge document round-tripping.**

The injected `LlmBackend` trait means all of this runs against a fake backend replaying recorded responses — no Apple frameworks, no model, no network, deterministic. The Swift layer is thin enough to need very little beyond view-model coverage.

## v1 scope

**In:** create a trip (destination plus travel window), fetch climate grounding, generate a grounded and validated packing list on-device one category at a time, check items off. Documents appear as a category. Automerge-backed storage with no transport yet.

**Out, by explicit decision:** sync of any kind, gear inventory, post-trip learning, buy-before-you-go list, carry-on and checked split, passport expiry watch, official-source document links, token-level streaming within a category, per-person item ownership in the interface.

## Roadmap

Sequenced roughly in build order.

1. **Sync transports** — CloudKit first, then Bonjour peer-to-peer over Network.framework for offline use on the same Wi-Fi, then AirDrop file exchange as the always-works manual fallback. All three are just moving bytes; the CRDT makes each of them safe.
2. **Post-trip learning** — mark what you never used, and future lists adapt. This is what makes Sundry compound in value rather than remain a nicer prompt wrapper.
3. **Gear inventory** — the app knows you own a universal adapter, so it says "pack it" rather than "buy one".
4. **Passport expiry watch** — warn when a passport falls within six months of expiry at the travel date, the most common reason travellers are refused entry. Pure deterministic logic.
5. **Buy-before-you-go list** — a genuinely different action from packing, and mixing them muddles both.
6. **Carry-on versus checked** — including liquid limits and airline weight allowance.
7. **Laundry-aware quantities** — ten nights is not ten shirts when there's a washing machine.
8. **Multi-climate trips** — one trip spanning Reykjavik and Tenerife, where a single flat list is actively wrong.
9. **iPhone companion** — mostly a new view layer, since the Rust core is already portable, and Foundation Models is available on iOS 26 too.

**Escape hatch, if on-device quality disappoints.** The `LlmBackend` trait is the whole mitigation. A stronger backend is a Swift-side swap behind an unchanged Rust pipeline, and Apple opened the Foundation Models framework in 2026 to drive external models through a common `LanguageModel` protocol, so a local Ollama model could be substituted without even leaving the framework. This stays out of scope unless grounded on-device output proves inadequate in practice — but it is deliberately cheap to reach for, which is why the trait exists in v1 rather than being retrofitted.
