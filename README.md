# Gestures

> This fork focuses on high-performance three-finger dragging with optimizations for both X11 and Wayland.
>
> For technical details, see: https://github.com/riley-martin/gestures/discussions/6
>
> Pre-compiled binaries: https://github.com/ferstar/gestures/releases
>
> Install via cargo: `cargo install --git https://github.com/ferstar/gestures.git`

## About
A libinput-based touchpad gesture handler that executes commands based on gestures.
Unlike alternatives, it uses the libinput API directly for better performance and reliability.

## Features
- **Platform Support**: Both X11 and Wayland
- **High Performance**:
  - X11: Direct libxdo API for minimal latency
  - Wayland: Optimized ydotool integration with 60 FPS throttling
  - Thread pool for command execution (4 workers, prevents PID exhaustion)
- **Gesture Types**: Swipe (8 directions + any), Pinch, Hold
- **Advanced Features**:
  - Mouse acceleration and delay for smooth 3-finger dragging
  - Real-time config reload via IPC
  - Graceful shutdown (SIGTERM/SIGINT)

## Configuration
See [config.md](./config.md) for detailed configuration instructions.

### Quick Setup
```bash
# Generate default config file
gestures generate-config

# Preview config without installing
gestures generate-config --print

# Force overwrite existing config
gestures generate-config --force
```

### Quick Example
```kdl
// 3-finger drag (works on both X11 and Wayland)
swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20

// 4-finger workspace switching
swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"
swipe direction="e" fingers=4 end="hyprctl dispatch workspace e+1"
```

## Installation

### Prerequisites
**System packages:**
- `libudev-dev` / `libudev-devel`
- `libinput-dev` / `libinput-devel`
- `libxdo-dev` / `libxdo-devel`

**Runtime dependencies:**
- X11: No extra runtime dependency for drag (uses `libxdo` directly)
- Wayland: `ydotool` + `ydotoold` daemon (for 3-finger drag)
  - If your distribution package has issues, try the official [ydotool binaries from GitHub releases](https://github.com/ReimuNotMoe/ydotool/releases)

### With Cargo
```bash
cargo install --git https://github.com/ferstar/gestures.git
```

### Manual Build
```bash
git clone https://github.com/ferstar/gestures
cd gestures
cargo build --release
sudo cp target/release/gestures /usr/local/bin/
```

### Nix Flakes
```nix
# flake.nix
{
  inputs.gestures.url = "github:ferstar/gestures";

  # Then add to packages:
  # inputs.gestures.packages.${system}.gestures
}
```

## Running

### Systemd (Recommended)
```bash
# 1. Generate config file (first time only)
gestures generate-config

# 2. Install service file
gestures install-service

# 3. Enable and start the service
systemctl --user enable --now gestures.service
```

### Manual
```bash
# Auto-detect display server (X11 or Wayland)
gestures start

# Force Wayland mode (if needed)
gestures --wayland start

# Force X11 mode (if needed)
gestures --x11 start

# Reload config
gestures reload

# Preview service file (without installing)
gestures install-service --print
```

**Note**: The display server (X11/Wayland) is automatically detected via `WAYLAND_DISPLAY` and `XDG_SESSION_TYPE` environment variables. Manual override is rarely needed.

## Performance Optimizations

This fork includes several performance improvements:

1. **Regex Caching**: One-time compilation using `once_cell::Lazy`
2. **Thread Pool**: 4-worker pool prevents PID exhaustion during fast gestures
3. **FPS Throttling**: 60 FPS limit for Wayland (considering ydotool ~100ms latency)
4. **Timer-based Delays**: Non-blocking mouse-up delays for smooth dragging
5. **Event Caching**: 1-second cache for gesture configuration lookups

## Troubleshooting

### High CPU on Wayland
- Default 60 FPS throttle should keep CPU <5%
- Adjust in `src/event_handler.rs` (`ThrottleState::new(60)`) if needed

### 3-Finger Drag Not Working
**X11:**
- Ensure X11 session env is correct (`DISPLAY` / `XAUTHORITY`)

**Wayland:**
- If your distribution package has issues, try the official [ydotool binaries from GitHub releases](https://github.com/ReimuNotMoe/ydotool/releases)
- Ensure `ydotoold` daemon is running: `systemctl --user status ydotoold`
- Configure uinput permissions (see [issue #4](https://github.com/ferstar/gestures/issues/4))

### `libxdo` Shared Library Error on X11
Symptom:
- `journalctl --user -u gestures` shows:
  - `error while loading shared libraries: libxdo.so.3: cannot open shared object file`

Cause:
- System `xdotool/libxdo` was upgraded (for example to `libxdo.so.4`), but your existing `gestures` binary was built against an older SONAME (`libxdo.so.3`).

Fix:
```bash
# Rebuild and reinstall gestures binary
cargo install --path . --force

# Restart user service
systemctl --user restart gestures

# Verify runtime link and logs
ldd ~/.cargo/bin/gestures | grep libxdo
journalctl --user -u gestures -n 50 --no-pager
```

### Conflicts with DE Gestures
Disable built-in gestures in your desktop environment (GNOME, KDE, etc.)

## Alternatives
- [libinput-gestures](https://github.com/bulletmark/libinput-gestures) - Parses debug output
- [gebaar](https://github.com/Coffee2CodeNL/gebaar-libinput) - Swipe only
- [fusuma](https://github.com/iberianpig/fusuma) - Ruby-based
