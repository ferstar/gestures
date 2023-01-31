> This fork repo mainly focus on the performance of three-figure-dragging feature.

> for more tech details, please vist this discussion: https://github.com/riley-martin/gestures/discussions/6

> My own compiled binaries and configuration can be found at https://github.com/ferstar/gestures/releases

> NOTE: ONLY WORKS ON X11, NO WAYLAND SUPPORT YET!
---
# Gestures
## About
This is a program for intercepting touchpad gestures and executing commands based on them.
Unlike some alternatives, it directly uses the libinput api rather than parsing the output
of `libinput debug-events`.

## Features
`gestures` is able to handle libinput swipe events; not only vertical and horizontal but diagonal
as well.
- [x] Handle libinput events
  - [x] Swipe events; vertical, horizontal and diagonal
  - [x] Pinch events
  - [x] Hold events
  - [ ] Rotate events
  - [x] Continuous and one-shot events
- [x] Config file

## Configuration
See [config.md](./config.md) for configuration instructions.

## Installation
### Platforms
Linux. The testing workflow runs on Ubuntu and I test it myself on Arch Linux, but it should work on any distro if it uses the
`libinput` touchpad driver rather than the older `synaptics` driver.  
Note: If your DE/WM has its own touchpad gestures system, it will most likely need to be disabled to
prevent conflicts.
### Dependencies
You may need to install `libudev` and `libinput`, or their equivalant for your distro, and possibly the `dev` versions as well.
### With Cargo
If you have cargo installed, simply use `cargo install gestures`
### Manual installation
- Clone the repo
  - `git clone https://github.com/riley-martin/gestures && cd gestures`

- Build
  - `cargo build --release`

- Copy `./target/release/gestures` to a convenient place and execute it
### Autostart
#### Systemd
Drop [examples/gestures.service](./examples/gestures.service) into `~/.config/systemd/user/gestures.service`
and modify it for your system (mainly the "$HOME" environment variable and the `ExecStart` will need changed).
To have it start automatically, run `systemctl --user enable --now gestures.service`.
#### Other init systems
I haven't used any other init systems, but the service is quite simple so it should be easy to modify
for other systems.

## Alternatives
Here are some alternatives that may suit your use case better, as well as the reasons I don't use them.

- [libinput-gestures](https://github.com/bulletmark/libinput-gestures)  
Parses `libinput debug-events` rather than using libinput api, which is less memory and cpu efficient
- [gebaar](https://github.com/Coffee2CodeNL/gebaar-libinput)
Not maintained, only supports swipe gestures
- [gebaar-libinput-fork](https://github.com/osleg/gebaar-libinput-fork)
Fork of gebaar which supports other gestures, but is also not actively developed
- [fusuma](https://github.com/iberianpig/fusuma)
Also parses `libinput debug-events` output


