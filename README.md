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

**Runtime dependencies:**
- X11: `xdotool` (for 3-finger drag)
- Wayland: `ydotool` + `ydotoold` daemon (for 3-finger drag)

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
# Copy service file
cp examples/gestures.service ~/.config/systemd/user/

# Edit paths in the service file
vim ~/.config/systemd/user/gestures.service

# Enable and start
systemctl --user enable --now gestures.service
```

### Manual
```bash
# X11
gestures start

# Wayland
gestures -w start

# Reload config
gestures reload
```

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
- Adjust in `src/event_handler.rs` line 89 if needed

### 3-Finger Drag Not Working
**X11:**
- Ensure `xdotool` is installed: `which xdotool`

**Wayland:**
- Ensure `ydotoold` daemon is running: `systemctl --user status ydotoold`
- Install ydotool: https://github.com/ReimuNotMoe/ydotool

### Conflicts with DE Gestures
Disable built-in gestures in your desktop environment (GNOME, KDE, etc.)

## Alternatives
- [libinput-gestures](https://github.com/bulletmark/libinput-gestures) - Parses debug output
- [gebaar](https://github.com/Coffee2CodeNL/gebaar-libinput) - Swipe only
- [fusuma](https://github.com/iberianpig/fusuma) - Ruby-based
