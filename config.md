# Gestures configuration
## Location
The configuration is looked for at `$XDG_CONFIG_HOME/gestures.kdl` and then at
`$XDG_CONFIG_HOME/gestures/gestures.kdl`. If `XDG_CONFIG_HOME` is not set, `$HOME/.config` is used
instead.

## Format
The configuration format (since 0.5.0) uses [`kdl`](https://kdl.dev).
```kdl
// Swipe requires a direction and fingers field at least
// direction can be one of "nw", "n", "ne", "w", "any", "e", "sw", "s", or "se"
// fingers is the number of fingers used to trigger the action
// start, update, and end are all optional. They are executed with `sh -c` and are executed when
// the gesture is started, recieves an update event and ends.
//
// In all of the fields which execute a shell command, `delta_x`, `delta_y` and `scale` are replaced
// with the delta in the x and y directions and the scale (movement farther apart or closer together)
// of the gesture. If they are used for an action in which they do not make sense (e.g. using 
// `scale` in the swipe gesture, 0.0 is used as the value.)
//

// For example, this will make a 3-finger-drag in any direction move the mouse(like the macOS 3-finger-drag)
// Your fingers can temporarily leave the touchpad for up to 500ms before the drag is cancelled.
// The acceleration is set to 20, which means that the mouse will move 20/10=2 times faster than your current mouse speed.
// NOTE: This 3-finger-drag config only works on x11,
// and it only works if you have xdotool installed.
swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20

// The below config may be working on wayland, but I haven't tested it.
// You need to install ydotool to use it.
// swipe direction="any" fingers=3 update="ydotool mousemove  -x $delta_x -y $delta_y" start="ydotool click -- 0x40" end="ydotool click -- 0x80"

swipe direction="w" fingers=4 end="xdotool key alt+Right"
swipe direction="e" fingers=4 end="xdotool key alt+Left"

// This will make a 4-finger swipe up open the application launcher
// (assuming you have a shortcut for it)
// The default shortcut for KDE may be "super+w"
swipe direction="n" fingers=4 update="" start="" end="xdotool key super+s"

// This will make a 4-finger swipe down close the current window
swipe direction="s" fingers=4 update="" start="" end="xdotool key ctrl+w"

// pinch direction can be "in" or "out". Other fields are the same as for
// the swipe gesture
pinch direction="in" fingers=4 end="xdotool key Ctrl+minus"
pinch direction="out" fingers=4 end="xdotool key Ctrl+plus"

// Hold only has one action, rather than start, end and update, because it does not
// make much sense to update it.
// hold fingers=4 action="xdotool key Super_L"
```
