# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A high-performance libinput-based touchpad gesture handler focused on optimizing three-finger dragging. Supports both X11 and Wayland display servers.

**Core Features:**
- Direct libinput API usage (no debug output parsing)
- X11: libxdo API for minimal latency
- Wayland: ydotool with 60 FPS throttling optimization
- Three gesture types: Swipe, Pinch, Hold
- Thread pool for command execution (4 workers, prevents PID exhaustion)
- Real-time config reload via IPC

## Build and Test

### Development Commands

```bash
# Build project (release version)
cargo build --release

# Run tests
cargo test

# Run a specific test
cargo test <test_name>

# Run with verbose logging
cargo run -- -vv start

# Lint and format checking
cargo fmt --all -- --check          # Check code formatting
cargo fmt --all                     # Auto-format code
cargo clippy --all-targets --all-features -- -D warnings  # Run clippy with warnings as errors

# Using Nix (if available)
nix build

# Development environment (Nix)
nix develop
```

### System Dependencies

**Build-time dependencies:**
- libudev-dev / libudev-devel
- libinput-dev / libinput-devel
- libxdo-dev / libxdo-devel

**Runtime dependencies:**
- X11 mode: xdotool (for 3-finger drag)
- Wayland mode: ydotool + ydotoold daemon (for 3-finger drag)

## Code Architecture

### Module Structure

```
src/
├── main.rs              # Entry point: CLI parsing, signal handling, display server detection
├── event_handler.rs     # Core event handler: libinput event loop, gesture recognition
├── mouse_handler.rs     # Mouse control abstraction: X11 (libxdo) vs Wayland (ydotool)
├── config.rs            # Configuration parsing (KDL format)
├── ipc.rs               # IPC server (Unix socket) for config reload
├── ipc_client.rs        # IPC client
├── utils.rs             # Command execution, variable substitution utilities
└── gestures/
    ├── mod.rs           # Gesture type definitions
    ├── swipe.rs         # Swipe gestures (8 directions + any)
    ├── pinch.rs         # Pinch gestures (in/out)
    └── hold.rs          # Hold gestures
```

### Key Design Patterns

**1. Display Server Auto-detection (main.rs:32-45)**
- Checks `WAYLAND_DISPLAY` environment variable (most reliable)
- Falls back to `XDG_SESSION_TYPE`
- Defaults to X11 if unable to detect
- Can be forced via `--wayland` or `--x11` flags

**2. MouseHandler Abstraction (mouse_handler.rs)**
- X11 mode: Creates dedicated thread running libxdo, communicates via mpsc channel
- Wayland mode: Directly invokes ydotool commands
- X11 initialization failure logs error but doesn't panic (allows fallback to Wayland mode)
- Uses Timer for non-blocking mouse-up delays (for 3-finger drag)

**3. Performance Optimizations (event_handler.rs)**
- **Gesture Cache** (GestureCache): Groups gesture configs by finger count, refreshes every second
- **FPS Throttling** (ThrottleState): 60 FPS limit for Wayland updates (accounting for ydotool ~100ms latency)
- **Regex Caching**: One-time compilation using `once_cell::Lazy` (utils.rs)
- **Thread Pool**: 4 worker threads for command execution (prevents PID exhaustion during fast gestures)

**4. IPC Config Reload (ipc.rs)**
- Creates Unix socket at `$XDG_RUNTIME_DIR/gestures.sock`
- Non-blocking mode, periodically checks SHUTDOWN flag
- Updates shared config using RwLock when "reload" command received

**5. Direct Mouse Control Detection (event_handler.rs:344-350)**
```rust
fn is_direct_mouse_gesture(gesture: &Gesture) -> bool {
    if let Gesture::Swipe(j) = gesture {
        j.acceleration.is_some() && j.mouse_up_delay.is_some() && j.direction == SwipeDir::Any
    } else {
        false
    }
}
```
This function identifies 3-finger drag gestures (direction="any" + mouse-up-delay + acceleration) to use direct mouse control instead of command execution.

### Configuration System

- Uses KDL format (via knuffel crate)
- Config search order:
  1. `$XDG_CONFIG_HOME/gestures.kdl`
  2. `$XDG_CONFIG_HOME/gestures/gestures.kdl`
  3. `~/.config/gestures.kdl`
- Supports variable substitution: `$delta_x`, `$delta_y`, `$scale`, `$delta_angle`

## Common Development Tasks

### Modifying Gesture Handling Logic

Main handler functions in `event_handler.rs`:
- `handle_swipe_event()` - Swipe gestures
- `handle_pinch_event()` - Pinch gestures
- `handle_hold_event()` - Hold gestures

### Adding New Gesture Types

1. Add new variant to `Gesture` enum in `src/gestures/mod.rs`
2. Create new module file in `src/gestures/`
3. Add handling branch in `handle_event()` in `event_handler.rs`
4. Update KDL parsing in `config.rs` (via Decode trait)

### Adjusting Performance Parameters

- **FPS Throttling**: Modify `ThrottleState::new(60)` in `event_handler.rs:89`
- **Cache Refresh Interval**: Modify `Duration::from_secs(1)` in `event_handler.rs:330`
- **Thread Pool Size**: Modify thread pool configuration in `utils.rs`

### Debugging

```bash
# View verbose logs
RUST_LOG=debug gestures start

# Or use command-line flags
gestures -vv start     # Very verbose
gestures -v start      # Info level
gestures -d start      # Debug level

# With systemd
journalctl --user -u gestures -f
```

## Important Constraints

1. **X11 Environment Detection** (mouse_handler.rs:24-73):
   - Automatically attempts to set `DISPLAY` and `XAUTHORITY`
   - Searches common locations: `~/.Xauthority`, `/tmp/xauth_*`
   - Gracefully degrades if libxdo initialization fails (logs warning but doesn't panic)

2. **Graceful Shutdown**:
   - Uses global `SHUTDOWN` atomic boolean flag
   - Registers SIGTERM and SIGINT signal handlers
   - Both event loop and IPC listener check shutdown flag

3. **3-Finger Drag Requirements**:
   - Must set both `mouse-up-delay` and `acceleration`
   - `direction` must be "any"
   - X11: Requires successful libxdo initialization
   - Wayland: Requires ydotoold daemon running

4. **Thread Safety**:
   - Config shared between threads using `Arc<RwLock<Config>>`
   - MouseHandler communicates with dedicated thread via mpsc channel
   - Uses `parking_lot::RwLock` (faster than std)

## Testing Strategy

- Unit tests located in `src/tests/mod.rs`
- Integration tests require touchpad device, typically manual testing
- Recommended manual testing workflow after modifying gesture logic:
  1. Generate config: `gestures generate-config`
  2. Start service: `gestures start`
  3. Test various gestures
  4. Modify config: Edit `~/.config/gestures.kdl`
  5. Reload: `gestures reload`

## CI/CD

- GitHub Actions workflows in `.github/workflows/`
- **CI Pipeline** (`ci.yml`):
  1. Lint: Runs `cargo fmt --check` and `cargo clippy` (warnings treated as errors)
  2. Test: Runs `cargo test` after lint passes
  3. Build Release: Builds release binary after tests pass
- Supports Nix builds (flake.nix)
- Automatically builds binaries and uploads to GitHub Releases on release

### Pre-commit Checks

Before committing code, ensure:
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
