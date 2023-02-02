# Gestures configuration
## Location
`$HOME/.config/gestures.conf`, `$HOME/.config/gestures/gestures.conf` and `$HOME/.gestures.conf`
are the configuration locations. They are read in that order, stopping whenever the first one is
encountered.
## Format
The configuration format is based on s-expressions.

```lisp
(
  ; device specifies which touchpad device to use. If left empty, selection is automatic.
  ; Currently HAS NO EFFECT
  ; (device)
  ; list of gestures. Available options are `swipe`, `pinch`, `hold` and `rotate`.
  ; Only `swipe` and `pinch` are currently implemented, others are ignored.
  ;
  ; All fields shown are required
  (gestures
    (swipe
      ; `direction`: can be N, S, E, W, NE, NW, SE, SW or any
      ; Three finger draging
      (direction . any)
      ; delay unit: ms
      (mouse_up_delay . (500))
      ; pointer acceleration
      (acceleration . (2))
      ; `fingers`: basically can be 3 or 4, because less than three libinput does not recognize
      ; as a gesture, and AFAICT more than four are not counted
      (fingers . 3)
      ; for Wayland 3-finger-drag feature, we need the ydotool's help
      (update . ("ydotool mousemove_relative -- $delta_x $delta_y"))
      ; `start`: command to execute on start event
      (start . ("ydotool click -- 0x40"))
      ; `end`: command to execute on end event
      (end . ("ydotool click -- 0x80"))
    )
    (swipe
      ; `direction`: can be N, S, E, W, NE, NW, SE, SW or any
      (direction . W)
      ; `fingers`: basically can be 3 or 4, because less than three libinput does not recognize
      ; as a gesture, and AFAICT more than four are not counted
      (fingers . 4)
      ; `action`: command to execute on update event. Anything that works with `sh -c` should work here.
      ; (update . ("xdotool mousemove_relative -- $delta_x $delta_y"))
      ; `start`: command to execute on start event
      ; `end`: command to execute on end event
      (end . ("xdotool key alt+Right"))
    )
    (swipe
      ; `direction`: can be N, S, E, W, NE, NW, SE, SW or any
      (direction . E)
      ; `fingers`: basically can be 3 or 4, because less than three libinput does not recognize
      ; as a gesture, and AFAICT more than four are not counted
      (fingers . 4)
      ; `action`: command to execute on update event. Anything that works with `sh -c` should work here.
      ; `end`: command to execute on end event
      (end . ("xdotool key alt+Left"))
    )
    (swipe
      ; `direction`: can be N, S, E, W, NE, NW, SE, SW or any
      (direction . N)
      ; `fingers`: basically can be 3 or 4, because less than three libinput does not recognize
      ; as a gesture, and AFAICT more than four are not counted
      (fingers . 4)
      ; `action`: command to execute on update event. Anything that works with `sh -c` should work here.
      ; (update . ("xdotool mousemove_relative -- $delta_x $delta_y"))
      (update . (""))
      ; `start`: command to execute on start event
      (start . (""))
      ; `end`: command to execute on end event
      (end . ("xdotool key super+s"))
    )
    (swipe
      ; `direction`: can be N, S, E, W, NE, NW, SE, SW or any
      (direction . S)
      ; `fingers`: basically can be 3 or 4, because less than three libinput does not recognize
      ; as a gesture, and AFAICT more than four are not counted
      (fingers . 4)
      ; `action`: command to execute on update event. Anything that works with `sh -c` should work here.
      ; (update . ("xdotool mousemove_relative -- $delta_x $delta_y"))
      (update . (""))
      ; `start`: command to execute on start event
      (start . (""))
      ; `end`: command to execute on end event
      (end . ("xdotool key super+s"))
    )
    (pinch
      ; same as above
      (fingers . 4)
      ; `direction`: in or out or any
      (direction . in)
      ; same as above
      (update . (""))
      ; same as above
      (start . (""))
      ; same as above
      (end . ("xdotool key Ctrl+minus"))
    )
    (pinch
      ; same as above
      (fingers . 4)
      ; `direction`: in or out or any
      (direction . out)
      ; same as above
      (update . (""))
      ; same as above
      (start . (""))
      ; same as above
      (end . ("xdotool key Ctrl+plus"))
    )
    ; hold action
    ; note that only oneshot is supported here
    (hold
      (fingers . 4)
      (action . ("xdotool key Super_L"))
    )
  )
)
  
```
