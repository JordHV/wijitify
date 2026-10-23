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

## Config shape (schema_version = 7)

```toml
schema_version = 7
setup_complete = true
theme = "dark" # or "light"; selecting a theme resets palette defaults
accent = "#3b82f6" # mirrored from palette.accent
datetime_format = "dd_mm_yy_am_pm"
font_family = "system" # system | sans | serif | mono
font_scale = "medium" # small | medium | large | x_large

[palette]
bg = "#0f1419"
surface = "#1a2332"
# … surface_2, surface_press, text, muted, accent, accent_press, danger, danger_press, ok, text_on_accent

[modules.zalender]
enabled = true
timezone = "America/New_York"
refresh_minutes = 15
next_feed_id = 2
week_start = "sunday"
hot_preset_id = "preset-month"
feeds = [{ id = "feed-1", name = "Family", url = "https://...", color = "#3b82f6", visible = true, writable = false }]

# Built-in presets: day, week, month, year, agenda (count = weeks for agenda)
[[modules.zalender.presets]]
id = "preset-month"
name = "Month"
unit = "month"
count = 1
anchor = "current_first"
builtin = true
visible = true

[modules.zalender.display]
dim_past = true
shade_weekends = true

[modules.zalender.day_modal]
show_time = true
show_title = true
show_calendar = true
show_description = true
```

Per-event customizations (color, icon, custom field values) live in `overrides.json` next to `config.toml`, not in the disposable ICS snapshot cache. See [customizationsmethod.txt](../customizationsmethod.txt).

See [PRINCIPLES.md](PRINCIPLES.md) and [BRIEF.md](BRIEF.md) for product constraints.
