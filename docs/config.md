# Gestures Configuration

## Location
Configuration file is searched in order:
1. `$XDG_CONFIG_HOME/gestures.kdl`
2. `$XDG_CONFIG_HOME/gestures/gestures.kdl`
3. `~/.config/gestures.kdl` (if XDG_CONFIG_HOME is unset)

## Format
Uses [KDL](https://kdl.dev) configuration language (since v0.5.0).

## Swipe Gestures

### Basic Syntax
```kdl
swipe direction="<dir>" fingers=<n> [start="<cmd>"] [update="<cmd>"] [end="<cmd>"]
```

**Parameters:**
- `direction`: `n`, `s`, `e`, `w`, `ne`, `nw`, `se`, `sw`, or `any`
- `fingers`: Number of fingers (typically 3 or 4)
- `start`: Command executed when gesture begins (optional)
- `update`: Command executed on each movement update (optional)
- `end`: Command executed when gesture ends (optional)

**Variable Substitution:**
In commands, these variables are replaced with actual values:
- `$delta_x`: Horizontal movement delta
- `$delta_y`: Vertical movement delta
- `$scale`: Pinch scale (for pinch gestures)
- `$delta_angle`: Rotation angle (for pinch gestures)

### 3-Finger Drag (macOS-like)

**Works on both X11 and Wayland:**
```kdl
swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20
```

**Parameters:**
- `mouse-up-delay`: Delay in milliseconds before releasing mouse button (allows finger to leave trackpad temporarily)
- `acceleration`: Mouse speed multiplier (20 = 2x speed, 10 = 1x speed)

**Requirements:**
- X11: No extra runtime dependency for drag (uses `libxdo` directly)
- Wayland: Install `ydotool` and run `ydotoold` daemon

**How it works:**
- X11: Uses libxdo API directly (minimal latency)
- Wayland: Uses timer-scheduled ydotool commands (optimized with 60 FPS throttling)

### Manual Wayland Control
If you prefer full control over Wayland commands:
```kdl
swipe direction="any" fingers=3 \
  start="ydotool click -- 0x40" \
  update="ydotool mousemove -x $delta_x -y $delta_y" \
  end="ydotool click -- 0x80"
```

### Workspace Switching Examples

**Hyprland:**
```kdl
swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"
swipe direction="e" fingers=4 end="hyprctl dispatch workspace e+1"
swipe direction="n" fingers=4 end="hyprctl dispatch fullscreen"
swipe direction="s" fingers=4 end="hyprctl dispatch killactive"
```

**i3/Sway:**
```kdl
swipe direction="w" fingers=4 end="i3-msg workspace prev"
swipe direction="e" fingers=4 end="i3-msg workspace next"
```

**GNOME:**
```kdl
swipe direction="n" fingers=4 end="gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval global.workspace_manager.get_active_workspace().get_neighbor(Meta.MotionDirection.UP).activate(global.get_current_time())"
```

## Pinch Gestures

### Syntax
```kdl
pinch direction="<in|out>" fingers=<n> [start="<cmd>"] [update="<cmd>"] [end="<cmd>"]
```

### Examples
```kdl
// Zoom in browser
pinch direction="out" fingers=2 end="xdotool key ctrl+plus"
pinch direction="in" fingers=2 end="xdotool key ctrl+minus"

// With continuous updates
pinch direction="out" fingers=2 \
  update="notify-send 'Scaling: $scale'"
```

## Hold Gestures

### Syntax
```kdl
hold fingers=<n> action="<cmd>"
```

### Examples
```kdl
// Show launcher
hold fingers=4 action="rofi -show drun"

// Screenshot
hold fingers=3 action="flameshot gui"
```

## Complete Example Configuration

```kdl
// 3-finger drag (X11 + Wayland)
swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20

// Workspace navigation
swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"
swipe direction="e" fingers=4 end="hyprctl dispatch workspace e+1"

// Application launcher
swipe direction="n" fingers=4 end="rofi -show drun"

// Close window
swipe direction="s" fingers=4 end="hyprctl dispatch killactive"

// Browser zoom
pinch direction="in" fingers=2 end="xdotool key ctrl+minus"
pinch direction="out" fingers=2 end="xdotool key ctrl+plus"

// App launcher on hold
hold fingers=4 action="rofi -show drun"
```

## Tips

1. **Test commands first**: Run commands manually before adding to config
2. **Reload config**: `gestures reload` (no restart needed)
3. **Wayland ydotool**: Ensure `ydotoold` daemon is running
4. **Disable DE gestures**: Prevent conflicts with built-in gestures
5. **Check logs**: Run `journalctl --user -u gestures -f` for debugging
