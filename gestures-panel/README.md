# Gestures Panel

MyGo native-UI control panel for [ferstar/gestures](https://github.com/ferstar/gestures) — a libinput touchpad gesture daemon.

This app does **not** rewrite the Rust engine. It edits the on-disk **KDL** config the daemon already uses, and starts/stops/reloads the `gestures.service` user systemd unit.

## Requirements

- Go **1.27+** (MyGo native UI)
- Linux (for systemd / tray); the binary still builds on other OSes
- Optional tray: `libayatana-appindicator3` on Linux
- The `gestures` binary installed and `gestures install-service` done once

## Build

```bash
cd gestures-panel
go build -o gestures-panel .
# or:
go tool mygo build
```

Headless CI / box without a display: `go build` and `go test` are enough.

## Run

```bash
./gestures-panel
# or during development:
go tool mygo dev
```

Config path (same search order as the Rust tool):

1. `$XDG_CONFIG_HOME/gestures.kdl`
2. `$XDG_CONFIG_HOME/gestures/gestures.kdl`
3. `~/.config/gestures.kdl`

## UI

**Tray** (if AppIndicator is available): Running/Stopped status, Open panel, Reload config, Quit.

**Service tab:** start / stop / reload `gestures.service`, display-server hint (X11/Wayland), `input` group check.

**Gestures tab:** list (type, fingers, direction, action), add / edit / delete. Save writes KDL then `systemctl --user reload gestures.service` (falls back to `gestures reload`).

### KDL shape (real ferstar/gestures schema)

```kdl
swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20
swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"
pinch direction="in" fingers=2 end="xdotool key ctrl+minus"
hold fingers=4 action="rofi -show drun"
```

Optional UI names are stored as `// name: …` comments above nodes.

> Note: some blog posts used a different `gesture "drag" swipe any { … }` shape. This panel targets the **actual** knuffel schema in ferstar/gestures (`swipe` / `pinch` / `hold` property nodes).

## Layout

```
gestures-panel/
  main.go                 # window + tray
  internal/model/         # gesture types
  internal/kdl/           # parse/format gestures.kdl
  internal/systemd/       # systemctl --user helpers + env checks
  internal/uiapp/         # MyGo native views
  testdata/sample.kdl
```

## Tests

```bash
go test ./...
```
