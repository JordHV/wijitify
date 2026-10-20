## Summary

<!-- What changed and why -->

## Principles checklist

- [ ] **Simple** — smallest change that works; no dead code; comments only for *why*
- [ ] **Perceived latency** — UI thread not blocked; touch feedback stays in Slint; navigation uses in-memory state
- [ ] **Offline first** — cache-friendly; atomic writes; no crash on corrupt/missing network
- [ ] **Speed** — no new idle polling; any complexity has a measured reason
- [ ] **Official docs** — APIs checked against docs for pinned versions

**Docs consulted:**

**New Rust concepts** (with Rust Book links, if any):

## Test plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-features`
