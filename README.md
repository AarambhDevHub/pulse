# Pulse

A btop-style system monitor for the terminal, written in Rust with [Ratatui](https://ratatui.rs).
One small binary for Linux, macOS and Windows.

- **Overview**: CPU history chart, per-core bars, memory and swap, download/upload sparklines
- **Processes**: sort, filter, scroll, and kill (with confirmation)
- **Network**: throughput chart and a per-interface table
- **Disks**: usage gauge for every mounted disk
- Themes (`dark`, `light`, `mono`) and a live-adjustable refresh rate
- A start-up splash: a rat walks in from the right, stands up, shakes its behind for three dance steps, then walks off to the left (any key skips it, `--no-splash` turns it off)

## Run

```bash
cargo run --release
cargo run --release -- --refresh 500 --theme light
cargo run --release -- --no-splash
```

Needs a terminal of at least 80x24.

## Keys

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab`, `1`-`4` | Switch tab |
| `↑` `↓` / `j` `k` | Move selection (Processes) |
| `PgUp` `PgDn`, `Home` `End` / `g` `G` | Page, first, last |
| `c` `m` `p` `n` | Sort by CPU / memory / PID / name (press again to reverse) |
| `/` | Filter by name, user or PID (`Enter` keeps it, `Esc` clears it) |
| `K` | Kill selected process (asks `y` / `n`) |
| `+` `-` | Faster / slower refresh |
| `?` | Help |
| `q`, `Ctrl+C` | Quit |

## How it works

```
 input thread ──Key/Resize──▶ ┐
                              ├─▶ main loop ─▶ App::handle_event ─▶ ui::render
 collector thread ─Snapshot─▶ ┘        ▲
        ▲                              │
        └────── Command::Kill ◀────────┘
```

- `collector.rs` owns `sysinfo` on its own thread, so a slow process scan never freezes the UI.
- `model.rs` holds plain data types, so the UI never touches `sysinfo`.
- `app.rs` is all state and key handling. It has no terminal code, so it is unit tested.
- `splash.rs` is the intro. The animation is a pure function of elapsed time, so the timeline is unit tested.
- `ui/` only draws. Render tests use Ratatui's `TestBackend`.

Details that are easy to get wrong, and handled here:

- CPU and network values are deltas, so the first real snapshot waits `MINIMUM_CPU_UPDATE_INTERVAL`.
- Network rates divide by the real elapsed time, not the nominal interval.
- Selection follows the PID, so re-sorting every tick does not move your cursor.
- Windows key-release events are ignored.
- The terminal is restored on panic.
- Pulse refuses to kill PID 1 or itself.

## Develop

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Dependencies

`ratatui` 0.30, `sysinfo` 0.40, `clap` 4, `color-eyre`. No async runtime is needed.
