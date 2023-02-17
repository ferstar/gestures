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
swipe direction="s" fingers=4 start="<command>" update="<command>" end="<command>"

// pinch direction can be "in" or "out". Other fields are the same as for
// the swipe gesture
//
pinch direction="out" fingers=3 start="<command>" update="<command>" end="<command>"

// Hold only has one action, rather than start, end and update, because it does not
// make much sense to update it.
hold fingers=3 action="<command>"
```
