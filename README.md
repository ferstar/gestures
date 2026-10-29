# Gestures

A libinput-based touchpad gesture handler for Linux that runs commands in response to gestures. It talks to the libinput API directly instead of parsing debug output, which keeps it fast and reliable on both X11 and Wayland.

This fork focuses on high-performance three-finger dragging, with optimizations for both X11 and Wayland. For the technical background, see the [upstream discussion](https://github.com/riley-martin/gestures/discussions/6). Pre-compiled binaries are available on the [releases page](https://github.com/ferstar/gestures/releases).

## Features

- **Platform support**: X11 and Wayland, auto-detected at startup
- **High performance**:
  - X11: direct libxdo calls for minimal latency
  - Wayland: ydotool integration with 60 FPS throttling
  - Thread pool for command execution (4 workers, prevents PID exhaustion)
- **Gesture types**: Swipe (8 directions + any), Pinch, Hold
- **Three-finger drag**: mouse acceleration and mouse-up delay for smooth dragging
- **Live config reload** over IPC, no restart needed
- **Graceful shutdown** on SIGTERM/SIGINT

## Installation

### Prerequisites

**System packages:**

- `libudev-dev` / `libudev-devel`
- `libinput-dev` / `libinput-devel`
- `libxdo-dev` / `libxdo-devel`

**Runtime dependencies:**

- X11: nothing extra for dragging (uses `libxdo` directly)
- Wayland: `ydotool` + the `ydotoold` daemon (for 3-finger drag)
  - If your distribution's package has issues, try the official [ydotool binaries from GitHub releases](https://github.com/ReimuNotMoe/ydotool/releases)

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

**Note**: The display server is detected via the `WAYLAND_DISPLAY` and `XDG_SESSION_TYPE` environment variables. Manual override is rarely needed.

## Performance Optimizations

This fork includes several performance improvements:

1. **Regex caching**: one-time compilation using `once_cell::Lazy`
2. **Thread pool**: a 4-worker pool prevents PID exhaustion during fast gestures
3. **FPS throttling**: mouse updates are limited to 60 FPS while accumulating skipped displacement and fractional pixels. Normal drag completion flushes remaining whole-pixel movement before releasing the button; cancellation discards pending movement.
4. **Timer-based delays**: non-blocking mouse-up delays for smooth dragging
5. **Event caching**: 1-second cache for gesture configuration lookups
6. **Mouse command lifecycle**: a dedicated worker serializes Wayland mouse commands and waits for each `ydotool` child to exit. Its bounded queue coalesces adjacent movement events without crossing button events. On graceful shutdown, the worker drains queued commands, releases any held drag button, and is joined.

## Troubleshooting

### High CPU on Wayland

- The default 60 FPS throttle should keep CPU under 5%
- Adjust in `src/event_handler.rs` (`ThrottleState::new(60)`) if needed

### 3-Finger Drag Not Working

**X11:**

- Ensure the X11 session environment is correct (`DISPLAY` / `XAUTHORITY`)

**Wayland:**

- If your distribution's package has issues, try the official [ydotool binaries from GitHub releases](https://github.com/ReimuNotMoe/ydotool/releases)
- Ensure the `ydotoold` daemon is running: `systemctl --user status ydotoold`
- Configure uinput permissions (see [issue #4](https://github.com/ferstar/gestures/issues/4))

### Wayland Permission Denied (trackpad or ydotool socket)

Symptoms:

- Gestures cannot read touchpad events
- Or logs contain ydotool socket permission/path errors

Checks and fixes:

```bash
# 1) Ensure current user is in input-related group (distro-dependent)
id -nG
sudo usermod -aG input $USER

# 2) Re-login so new group membership takes effect

# 3) Run ydotoold as user service (not system/root service)
systemctl --user enable --now ydotoold.service
systemctl --user status ydotoold.service

# 4) Verify socket exists in user runtime dir
ls -la "$XDG_RUNTIME_DIR/.ydotool_socket"
```

Notes:

- Avoid `chmod 777` on the socket as a long-term fix.
- Keep `gestures` and `ydotoold` in the same user session to avoid permission mismatch.

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
