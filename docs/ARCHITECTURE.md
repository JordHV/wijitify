# Architecture

Wijitify is a **modular monolith**: one native Rust + Slint executable with a shell, shared services, and statically registered modules. **Zalender** is the first module (calendar).

## Names

| Name | Role |
| --- | --- |
| **Wijitify** | App shell, registry, setup, packaging, binary |
| **Zalender** | Calendar module (`id = "zalender"`) |

## Layout

```text
Full-screen Slint shell
  ├── First-run setup (module picker → per-module setup pages)
  ├── Navigation / home / settings
  └── Module view hosting
             │
     Module registry + lifecycle API
             │
        Zalender (…future modules)
             │
Shared services: config, storage, tasks, logging
```

## Threading

- **UI thread:** Slint event loop only. Updates in-memory models. Never blocks on I/O.
- **Background:** Tokio runtime on a dedicated thread. Network, disk, ICS parse/rrule.
- Results return via `slint::invoke_from_event_loop` / `Weak::upgrade_in_event_loop`.

## Module contract

Modules implement `Module` (`meta`, `start`, `stop`) and receive a `ModuleContext` (config slice, cache dir, task spawner, status reporter). They are compiled in and registered statically. Enable/disable is runtime visibility/lifecycle, not dynamic loading.

Each module ships Slint **views** and a **setup page**. The shell switches by module id.

## First-run setup

If `setup_complete` is false (or no config): pick modules → walk setup pages for selected modules → atomic save with `setup_complete = true` → start enabled modules → home.

## Config shape (schema_version = 1)

```toml
schema_version = 1
setup_complete = true

[modules.zalender]
enabled = true
timezone = "America/New_York"
refresh_minutes = 15
feeds = [{ name = "Family", url = "https://...", color = "#3b82f6" }]
```

See [PRINCIPLES.md](PRINCIPLES.md) and [BRIEF.md](BRIEF.md) for product constraints.
