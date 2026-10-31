# Gestures

English | [简体中文](./README.zh-CN.md)

A libinput-based touchpad gesture handler for Linux that runs commands in response to gestures. It talks to the libinput API directly instead of parsing debug output, which keeps it fast and reliable on both X11 and Wayland.

This fork focuses on high-performance three-finger dragging, with optimizations for both X11 and Wayland. For the technical background, see the [upstream discussion](https://github.com/riley-martin/gestures/discussions/6). Pre-compiled binaries are available on the [releases page](https://github.com/ferstar/gestures/releases).

## Features

- **Platform support**: X11 and Wayland, auto-detected at startup
- **High performance**:
  - X11: direct libxdo calls for minimal latency
  - Wayland: direct uinput writes through a persistent virtual pointer, with ydotool fallback
  - Thread pool for command execution (4 workers, prevents PID exhaustion)
- **Gesture types**: Swipe (8 directions + any), Pinch, Hold
- **Three-finger drag**: macOS-like drag with configurable mouse-up delay
- **Live config reload** over IPC, no restart needed
- **Graceful shutdown** on SIGTERM/SIGINT
- **Control panel**: optional GUI in `apps/panel` to edit gestures and manage the user service

## Installation

### Prerequisites

**System packages:**

- `libudev-dev` / `libudev-devel`
- `libinput-dev` / `libinput-devel`
- `libxdo-dev` / `libxdo-devel`

**Runtime dependencies:**

- X11: nothing extra for dragging (uses `libxdo` directly)
- Wayland: read/write access to `/dev/uinput` for the default direct-input backend. `ydotool` + `ydotoold` are only needed if uinput initialization fails or custom commands explicitly use ydotool.
  - See the permission setup below; no daemon is required for the default uinput backend.

### With Cargo

```bash
cargo install --git https://github.com/ferstar/gestures.git
```

### Manual Build

```bash
git clone https://github.com/ferstar/gestures
cd gestures
cargo build --release   # workspace root; engine lives in crates/gestures
sudo cp target/release/gestures /usr/local/bin/

# or install from a local checkout
cargo install --path crates/gestures --force
```

### Control Panel (`apps/panel`)

Optional GUI for editing gestures and managing the user service. Requires Go 1.27+.

```bash
cd apps/panel
go build -o gestures-panel .
./gestures-panel
```

See [apps/panel/README.md](./apps/panel/README.md) for details.

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

See [docs/config.md](./docs/config.md) for detailed configuration instructions.

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
swipe direction="any" fingers=3 mouse-up-delay=500

// 4-finger workspace switching
swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"
swipe direction="e" fingers=4 end="hyprctl dispatch workspace e+1"
```

### Touchpad Acceleration for This Process

Optionally match the acceleration profile and speed used by your desktop:

```kdl
touchpad accel-profile="adaptive" accel-speed=0.3
swipe direction="any" fingers=3 mouse-up-delay=500
```

Both properties are optional: `accel-profile` accepts `"adaptive"` or `"flat"`; `accel-speed` accepts a finite number from `-1.0` (slowest) to `1.0` (fastest), including `0.0` (device default speed). Invalid values reject the configuration.

These settings apply only to gesture movement in this program's independent libinput context (dragging and swipe `$delta_x`/`$delta_y`). They do not change the compositor's system pointer settings. (Note: legacy swipe `acceleration` property is deprecated and ignored).

`gestures reload` applies changed settings to existing devices, even while idle; newly connected gesture devices receive the current settings. Unspecified properties are left untouched (libinput defaults on startup). To restore a previously overridden setting, set its desired value explicitly, or remove the override and restart gestures.

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
3. **Input-driven dragging**: direct mouse movement follows touchpad events without a fixed frame-rate limit, retaining fractional pixels for slow-drag precision. The worker coalesces queued movements when needed. Custom swipe update commands remain limited to 60 updates per second.
4. **Timer-based delays**: non-blocking mouse-up delays for smooth dragging
5. **Event caching**: 1-second cache for gesture configuration lookups
6. **Mouse command lifecycle**: a dedicated worker owns a persistent uinput virtual pointer and writes movement/button events directly. If uinput initialization fails, it falls back to ydotool and waits for each child to exit. Its bounded queue coalesces adjacent movement events without crossing button events. On graceful shutdown, the worker drains queued commands, releases any held drag button, destroys its virtual device on exit, and is joined.

## Troubleshooting

### High CPU on Wayland

- Direct dragging has no fixed frame-rate limit; its bounded worker queue coalesces adjacent movement events.
- Custom swipe update commands are limited by `ThrottleState::new(60)` in `crates/gestures/src/event_handler.rs`.
- Check service CPU usage and logs while dragging; resource use depends on the touchpad event rate and command execution speed.

### 3-Finger Drag Not Working

**X11:**

- Ensure the X11 session environment is correct (`DISPLAY` / `XAUTHORITY`)

**Wayland:**

- The default backend writes directly to `/dev/uinput`; `ydotoold` is not required.
- Check logs for the selected backend. Failure to create the virtual pointer logs the reason and falls back to ydotool.
- Ensure the uinput module is loaded and your user can read/write `/dev/uinput`.
- If using the fallback, install `ydotool`, run `ydotoold`, and check its socket permissions.

### Wayland Permission Denied (trackpad or uinput)

Touchpad event access and virtual pointer creation both need appropriate permissions. On distributions using an `input` group:

```bash
# Load the kernel module if /dev/uinput is missing
sudo modprobe uinput

# Add your user to the group (log out and back in afterward)
sudo usermod -aG input "$USER"

# Inspect device ownership and permissions
ls -l /dev/uinput /dev/input/event*
```

If `/dev/uinput` is not group-writable, create `/etc/udev/rules.d/99-gestures-uinput.rules` with:

```udev
KERNEL=="uinput", GROUP="input", MODE="0660"
```

Use your distribution's `uinput` group instead if applicable. Input-group membership also grants access to other input devices. Apply the rule and restart gestures after logging back in:

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=misc --sysname-match=uinput
systemctl --user restart gestures.service
```

Only when using the ydotool fallback (or explicit ydotool custom commands):

```bash
systemctl --user enable --now ydotoold.service
systemctl --user status ydotoold.service
ls -la "$XDG_RUNTIME_DIR/.ydotool_socket"
```

Keep `gestures` and `ydotoold` in the same user session for the fallback. Avoid world-writable device or socket permissions.

### `libxdo` Shared Library Error on X11

Symptom:

- `journalctl --user -u gestures` shows:
  - `error while loading shared libraries: libxdo.so.3: cannot open shared object file`

Cause:

- System `xdotool/libxdo` was upgraded (for example to `libxdo.so.4`), but your existing `gestures` binary was built against an older SONAME (`libxdo.so.3`).

Fix:

```bash
# Rebuild and reinstall gestures binary
cargo install --path crates/gestures --force

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
