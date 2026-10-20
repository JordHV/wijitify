# Wijitify guiding principles

Every change to Wijitify and its modules follows these principles. When they conflict, use this order: **correctness → perceived latency → offline behavior → raw speed → official documentation**. Simplicity is how each of them is implemented: pick the simplest design that meets the higher priority.

## 1. Simple code that works

- Write the smallest code that solves today's problem. Add an abstraction (trait, generic, extra layer) only when a second real use exists. The `Module` trait is the planned exception.
- Prefer `std` and Slint built-ins. Every new crate needs a one-line reason in the PR. Use `default-features = false` and enable only the features you need.
- Keep functions small and single-purpose. Keep module nesting flat. No speculative config options, commented-out code, or dead code.
- Comments explain *why* or a non-obvious constraint, never *what* the next line does. Doc comments are required on the module contract and shared service APIs.
- Use `anyhow` with `?`. Non-test code has no `unwrap()`; use `expect("reason")` only for proven invariants.

**Checked by:** clippy (`-D warnings`, `clippy::unwrap_used` outside tests), PR review against this list.

## 2. Lowest possible perceived latency

- Every touch shows visible feedback by the next frame. Pressed/highlight states live in Slint, not a round trip through Rust.
- The UI thread never does network, disk, parsing, image decoding, sleeps, or waits on a contended lock. UI callbacks should finish in under ~2 ms; anything longer moves to the background runtime.
- Navigation is instant and reads only in-memory state. Data fills in afterwards and never blocks the view change.
- When cached data exists, do not show a blocking spinner. Show stale data with a small refresh indicator.
- Background work is cancellable and coalesced (only the latest request runs). Results apply as small model updates, not full rebuilds that hitch the UI.
- Animations stay at 150 ms or less and never delay input.

**Checked by:** `WIJITIFY_PERF=1` logs frames over 16.7 ms and UI callbacks over 2 ms; stress tests while a refresh runs.

## 3. Offline first, judicious local storage

- The local cache is the source of truth for the UI. The network only refreshes it. The app starts from cache and is usable with no network.
- Store only what improves the experience: each feed's last good raw ICS plus ETag, and a versioned snapshot of parsed events for fast startup. Prune outside the display window; bound each module's cache size.
- Writes are atomic (temp file, then rename). Config saves are debounced. Nothing writes to disk in a hot loop.
- A corrupt or old-version cache is discarded and fetched again; it never crashes the app.
- Offline is a normal state. Show "last updated", retry with exponential backoff, refresh when the network returns.

**Checked by:** offline integration test (network disabled, start from cache).

## 4. Speed

- Release profile: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `strip = true`.
- Do the work once, in the background. Precompute per-day event lists so the UI only does indexed lookups.
- When idle: no polling, no per-frame timers. Timers are event-driven (for example the clock ticks once per minute, aligned to the minute).
- Starting budgets (refine after measuring on the Surface, not the VM):
  - first frame under 1 s from cold start
  - ~0% CPU when idle
  - under 150 MB steady memory
  - no frames over 16.7 ms during navigation while a refresh runs
- Measure before optimizing. Extra complexity needs a before-and-after number.

**Checked by:** `WIJITIFY_PERF=1` and Surface measurements recorded in release notes when claiming a win.

## 5. Grounded in official documentation (lowest priority)

Base Rust and dependency usage on official sources, not memory or blog posts. Preference order:

1. [The Rust Book](https://doc.rust-lang.org/book/), [`std` docs](https://doc.rust-lang.org/std/), [Cargo Book](https://doc.rust-lang.org/cargo/), [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
2. [Clippy lint docs](https://rust-lang.github.io/rust-clippy/)
3. Crate docs on [docs.rs](https://docs.rs) for the version in `Cargo.lock`
4. [Slint docs](https://docs.slint.dev), [Tokio tutorial](https://tokio.rs/tokio/tutorial), [Flatpak docs](https://docs.flatpak.org)

- If docs and an AI suggestion disagree, the docs win.
- Prefer the idiomatic pattern the docs show. Stable Rust only; no nightly features.
- Put doc links in the PR description, not in code comments, unless the code depends on a non-obvious documented constraint.
- Each PR summary names new Rust concepts (ownership, lifetimes, traits, `async`, …) with a Rust Book link.
- Offline: `cargo doc --open` matches pinned dependency versions. See [DEVELOPMENT.md](DEVELOPMENT.md) for the link list.

**Checked by:** PR template "Docs consulted" and "New Rust concepts" lines.
