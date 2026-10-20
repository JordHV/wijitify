# Modular Linux Touchscreen Dashboard — Project Brief

**Status:** Initial requirements and architecture; application not yet implemented  
**Project focus:** Native Rust + Slint touchscreen calendar, designed to expand into a modular home dashboard  
**Updated:** October 2026

## 1. Vision

Create an **open-source, extremely responsive, full-screen Linux touchscreen application** for a dedicated Microsoft Surface Book 3 tablet. The first useful release is a **calendar dashboard**; photo-display/screensaver functionality is an early companion feature. Over time, the same application shell should support additional optional modules, eventually including smart-home controls.

The project is intended to be **developed and maintained primarily with AI assistance**. The owner has Python programming experience but is new to Rust. Favor clear boundaries, tests, documentation, automated builds, and simple operational workflows over unnecessary framework complexity.

## 2. Confirmed technical direction

| Area | Direction |
| --- | --- |
| Device | Microsoft **Surface Book 3**, tablet-only, dedicated display |
| Operating system | **Ubuntu 24.04 LTS Desktop** as initial base; use the `linux-surface` kernel and touchscreen support (`iptsd`) if needed |
| Display model | **Fixed, full-screen touchscreen UI**; not a conventional resizable desktop app |
| Core language | **Rust**, compiled native application |
| UI framework | **Slint** (declarative native UI); validate its rendering/touch behavior under the chosen Linux Wayland environment |
| Architecture | **Modular monolith**: one native executable with separate built-in modules and a shared application shell |
| Initial functionality | Calendar first; photo display/screensaver as an early feature |
| Future functionality | Optional modules for smart-home dashboard/control; deliberately **out of scope for V0 and V2** |
| Source hosting | Public **GitHub repository** with version history, issues, releases, documentation |
| Automation | **GitHub Actions** for checks, testing, packaging, and versioned release publication |
| Linux distribution | Prefer **Flatpak** for cross-distribution app delivery; initial target is Ubuntu, but Fedora and other Flatpak-capable distributions should be practical later |
| Updates | App detects when a newer release exists and notifies the user; **Flatpak** handles downloading/installing application updates, not a custom self-replacing updater |

**Product principle:** Optimize for a fast, dependable appliance, not for supporting every possible desktop environment or plug-in mechanism in the first releases.

## 3. Core requirements and non-goals

### Responsiveness — highest priority

- Touch interactions should generate **immediate visible feedback**. Aim for the next display frame; at 60 Hz the frame budget is ~16.7 ms. This is an engineering target, not a guaranteed end-to-end latency.
- UI rendering and event handling **must never wait synchronously** on calendar sync, network access, image decoding, file scans, updates, or disk I/O.
- Maintain current calendar/display state in memory; load and cache data in the background.
- Preload/decode slideshow images ahead of display changes.
- Target smooth 60 FPS interactions where supported; measure frame timing and stress-test while background services are active.
- Minimize unnecessary runtimes/services and monitor long-running memory and CPU use.

### Appliance operation

- Start at boot and enter the full-screen application automatically, with a safe way to exit for maintenance.
- Run reliably for days/weeks; survive temporary network loss and restart cleanly after application failure or system reboot.
- Allow offline viewing of locally cached calendar information and photos where practical.
- Configure power management appropriately for a plugged-in, always-on display (screen blanking, suspend, brightness, thermal checks).
- Preserve accessible recovery/maintenance pathways during initial development; do **not** start by stripping Ubuntu down to an irreversible kiosk-only image.

### Scope boundaries

- **Do not** build actual lighting, thermostat, camera, or smart-home integrations in V0 or V2.
- **Do not** build a general-purpose runtime plug-in loader or third-party plug-in marketplace initially.
- **Do not** design a fleet-management service or custom app updater initially.
- A future dynamic plug-in ABI, inter-process architecture, or third-party extension SDK is a separate decision, not a prerequisite for a clean module design.

## 4. Application architecture

Use a **modular monolith** with a central shell and small, self-contained built-in modules.

```text
Full-screen Slint application shell
  ├── Navigation / menu / dashboard layout
  ├── Settings / enabled-module toggles
  └── Module view hosting
             │
     Module registry + lifecycle API
             │
  ┌──────────┼───────────┐
  │          │           │
Calendar   Photos     Future modules
  │          │         (disabled/not shipped)
  └──────────┼───────────┘
             │
Shared services: config, storage, background tasks,
logging, notifications, update status
```

### Shell responsibilities

- Own the global layout, navigation, settings surface, lifecycle, and error recovery.
- Present available modules; allow each shipped module to be enabled or disabled with a checkbox/toggle.
- Route user interaction to modules without knowing their internal business logic.
- Provide shared services rather than encouraging copy-pasted network/storage/notification code.

### Built-in module contract

Design a small, typed Rust interface (e.g., traits + metadata) that can answer:

1. Module identifier, name, description and version/capabilities.
2. Whether the module is available and enabled.
3. Whether it provides a primary dashboard view and/or a configuration view.
4. Initialization, activation, deactivation, shutdown, and error reporting.
5. Access to narrowly scoped shared services, not arbitrary internal state from other modules.

**Modules are compiled into the app** and registered statically at first. Enabling/disabling controls runtime behavior and visibility; it **does not dynamically install or remove code**. Later module additions arrive as normal app releases.

### Development guardrails

- Keep UI components separate from application logic and integrations.
- No blocking network/disk/image work on the UI thread; use cancellable background operations and send results back to UI state.
- Avoid direct access across module internals. Define intentional interfaces and shared data contracts.
- Use typed, versioned configuration with migrations when schemas change.
- Log errors without exposing secrets; build graceful fallback states for offline/unavailable integrations.
- Prefer a small module API over speculative generality.

## 5. Functional roadmap

**V0 — Small working appliance (calendar-first)**

- Bootable full-screen Slint shell with touch navigation and basic menu/settings.
- Calendar view, month/day navigation, and basic event display; determine calendar data source/API separately.
- Shared settings persistence; basic module registry and module enable/disable controls for modules actually shipped.
- Graceful loading/empty/error states; asynchronous data retrieval.
- Automatic startup, error logging/recovery, and basic performance instrumentation.
- GitHub CI, release builds, Flatpak packaging, documentation, and first reproducible install.

**Early follow-on — Photo display/screensaver**

- Full-screen or integrated photo display (final presentation to be decided).
- Configurable image sources, rotation interval, and screensaver/idle behavior.
- Background preloading/caching so image transitions do not stall rendering.

**V1–V2 — Improve the core, not smart-home breadth**

- Refine touch navigation, polish settings/configuration, harden offline behavior and recovery.
- Improve calendar integration and reliability; test extended runtime.
- Expand module lifecycle/tests as real requirements emerge.
- Improve release/update UX and distribution documentation.

**Beyond V2 — Smart-home expansion**

- Add optional modules such as lighting, climate, sensor status, cameras, or Home Assistant integration.
- Reuse the existing shell/menu/settings and enable/disable semantics.
- Reassess per-module permissions, credentials, remote API design, and whether independently deployed integrations are actually needed.

The roadmap describes **intended order, not fixed version promises**. Calendar and photos may be combined in an early milestone if development progress allows.

## 6. GitHub repository strategy

One public repository is sufficient initially. Keep application, tests, packaging metadata, release automation, and architecture documentation together.

```text
project-root/
├── Cargo.toml
├── Cargo.lock                 # commit for application reproducibility
├── src/
│   ├── main.rs
│   ├── app/                    # shell state, navigation, lifecycle
│   ├── modules/
│   │   ├── registry.rs
│   │   ├── calendar/
│   │   └── photos/             # when implemented
│   └── services/               # config, logging, task runner, storage
├── ui/                         # Slint layout/components
├── assets/                     # icons, app assets
├── tests/                      # integration / behavior tests
├── packaging/
│   └── flatpak/                # Flatpak manifest, desktop metadata
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── docs/
│   ├── ARCHITECTURE.md
│   ├── DEVELOPMENT.md
│   └── RELEASES.md
├── README.md
├── CHANGELOG.md
└── LICENSE                      # choose an OSI-approved license
```

Suggested Git approach:

- Work locally, commit/push changes to GitHub; use `main` as the integration branch.
- **Prefer feature branches and pull requests** for substantial AI-generated changes, even as a solo developer, so required CI checks can catch regressions before merging. For tiny changes direct pushes can be allowed if desired.
- Record version history in Git; tag releases as `v0.1.0`, `v0.2.0`, etc. Never repurpose a published release tag.
- Keep release notes and a changelog documenting changes, bug fixes, and migration considerations.

## 7. GitHub Actions CI/CD pipeline

### CI: on pushes to `main` and pull requests

1. Checkout source and install pinned/stable Rust toolchain and required Linux/Slint dependencies.
2. Restore/cache Cargo dependencies and build artifacts where appropriate.
3. Run `cargo fmt --all -- --check`.
4. Run `cargo clippy --all-targets --all-features -- -D warnings`.
5. Run `cargo test --all-features` (plus integration tests).
6. Run `cargo build --release`.
7. Build/validate Flatpak packaging, initially on an appropriate Linux GitHub runner; add installation/smoke checks where feasible.
8. Fail the run on regressions. Store logs and optional test/build artifacts.

### Release: on a version tag (e.g., `v0.1.0`)

1. Verify tag/version consistency; rerun required checks.
2. Build optimized Rust binary.
3. Build and validate a Flatpak package using pinned runtime/SDK versions.
4. Produce immutable, versioned distribution artifacts and checksums.
5. Publish a **GitHub Release** with changelog/release notes and assets.
6. Publish/update the **Flatpak distribution channel** (see below) using separately scoped credentials.
7. Optionally smoke-test the install on Ubuntu; expand target testing over time.

**Important:** A GitHub Release containing a `.flatpak` or `.flatpakref` is not by itself a continuously updateable Flatpak repository. Updates require a configured **Flatpak remote** (e.g., Flathub or your own OSTree-backed Flatpak repository) to which new builds are published.

## 8. Flatpak distribution and updates

**Why Flatpak:** Distribute the same application across Ubuntu/Fedora and other supported Linux systems without producing and maintaining separate `.deb` and `.rpm` packages. It also provides runtime dependency management and a standard update mechanism.

### Practical launch options

- **Option A — Flathub (long-term, public distribution):** Submit the app for review, meet metadata/security requirements, and use Flathub's build/publishing process. Your GitHub Actions release workflow coordinates source tags and release metadata; Flathub may build from its own manifest/repository process.
- **Option B — Self-hosted Flatpak remote (useful for private/early distribution):** Build/publish a Flatpak OSTree repository, host it over HTTPS, and configure that remote on the Surface. This requires hosting, signing/key management, publishing, and a maintenance plan.
- **Option C — Downloadable Flatpak bundle for early manual testing:** Upload `.flatpak` bundles to GitHub Releases. Easy for first installs/tests, **but not sufficient for automatic ongoing updates** unless a proper remote is also configured.

**Recommended progression:** Make a Flatpak bundle for V0 validation, then choose Flathub or a properly maintained custom Flatpak remote before advertising in-app update/install support.

### Update UX and ownership

- The application may check the **GitHub Releases API** for version/release notes and display “Update available.” This is a **notification source**, not the system of record for installed Flatpak updates.
- For an installed Flatpak, use the **configured Flatpak remote / system update service** to determine and install the actual available update. Updates may be managed by GNOME Software, a scheduled Flatpak update process, or a carefully permissioned updater helper.
- Do **not** have the sandboxed app overwrite its own executable or assume it can execute privileged `flatpak update` unattended. Any install action needs an explicit, secure host-side mechanism; otherwise notify the user and let the OS update system apply the update.
- Allow application restart when the update has been applied. Make migrations/versioned settings safe and rollback planning explicit.
- Use signed, trusted releases/remote metadata, minimal secrets, and protected release publishing.

## 9. Surface Book 3 deployment considerations

- Install Ubuntu 24.04 LTS Desktop first; update fully within 24.04 before kiosk hardening.
- Test touch, Wi-Fi, display scaling, brightness, charging, power consumption, and reboot reliability.
- Use Linux Surface kernel and `iptsd` for Surface-specific touch behavior if required; confirm real behavior rather than assuming live-USB compatibility.
- Camera support is not an initial requirement; keyboard-base NVIDIA GPU is irrelevant for tablet-only use.
- Validate Slint rendering and input on Wayland, native high-DPI display, and any orientation/scale setting.
- Configure graphical auto-login / app autostart once basic tests pass; keep administrator access for maintenance.
- Secure Boot and bootloader issues are installation concerns, **not application requirements**; restore a secure supported configuration where feasible after validating Ubuntu/Linux Surface.

## 10. Initial implementation sequence

1. **Stabilize the target OS/hardware:** Ubuntu, Surface touch stack, Wi-Fi, graphics, power, reboot.
2. **Create the GitHub repository:** license, README, architecture brief, issue milestones and Rust skeleton.
3. **Build the Slint shell:** full-screen launch, responsive navigation, menu and settings placeholders.
4. **Define the minimal module registry/contract:** register Calendar as the first built-in module; persist enabled state.
5. **Implement calendar end-to-end:** views, state, basic event source, async refresh and offline handling.
6. **Measure UI responsiveness:** next-frame touch feedback, navigation stress tests, memory use, background-work isolation.
7. **Implement CI:** formatting, linting, tests, release build, Flatpak packaging validation.
8. **Ship a tagged V0 GitHub Release:** Flatpak test bundle and installation docs.
9. **Choose and configure an updateable Flatpak remote:** Flathub or custom; test actual upgrades and restart behavior.
10. **Add Photos and later modules** without restructuring the shell or introducing dynamic plugins prematurely.

## 11. Open decisions to resolve during implementation

- Calendar provider(s) and authentication approach (local calendar, CalDAV, Google/Microsoft integration, etc.).
- Final screen design: calendar and photos in one layout vs photos as a full-screen idle/screensaver mode.
- Exact app identifier, repository name, license, versioning and public release cadence.
- Chosen Flatpak publishing method (Flathub vs own remote), signing, and how the UI offers safe install/restart choices.
- Slint renderer/backend, desktop/session configuration, display scaling, rotation, and touch usability on the actual Surface Book 3.
- Security/credential storage strategy for eventual calendar accounts and smart-home integrations.

---

**Guiding rule for implementation and AI contributions:** Deliver a small, fast calendar appliance now. Preserve clean module boundaries so photos and eventual smart-home components can be added later. Use GitHub Actions, reproducible Rust builds, versioned GitHub Releases, and a real Flatpak update channel—without inventing complex infrastructure before it is needed.
