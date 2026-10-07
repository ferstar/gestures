// Package model holds the gesture configuration types matching ferstar/gestures.
package model

import (
	"fmt"
	"strings"
)

// Kind is the on-disk gesture node name: swipe, pinch, or hold.
type Kind string

const (
	KindSwipe Kind = "swipe"
	KindPinch Kind = "pinch"
	KindHold  Kind = "hold"
)

// ActionMode is a UI convenience: drag (mouse drag) vs exec (run a command).
type ActionMode string

const (
	ModeDrag ActionMode = "drag"
	ModeExec ActionMode = "exec"
)

// Gesture is one configured gesture, matching the Rust knuffel schema.
type Gesture struct {
	// Name is optional UI/label text; written as a // comment above the node.
	Name string

	Kind      Kind
	Direction string // swipe: any|n|s|e|w|ne|nw|se|sw; pinch: in|out|clockwise|counterclockwise|any; hold: unused
	Fingers   int

	// Swipe drag fields (mouse-up-delay / acceleration).
	Acceleration  *int   // typically 1–40; Rust uses i8
	MouseUpDelay  *int64 // milliseconds

	// Command fields (single shell-ish string, as the Rust tool expects).
	Start  string
	Update string
	End    string // swipe/pinch
	Action string // hold
}

// Config is the full gestures.kdl document.
type Config struct {
	Gestures []Gesture
	// HeaderComments are leading // lines preserved on round-trip when possible.
	HeaderComments []string
}

// Mode returns drag if this swipe looks like a mouse-drag, else exec.
func (g Gesture) Mode() ActionMode {
	if g.Kind == KindSwipe && (g.Acceleration != nil || g.MouseUpDelay != nil) &&
		g.End == "" && g.Start == "" && g.Update == "" {
		return ModeDrag
	}
	if g.Kind == KindSwipe && (g.Acceleration != nil || g.MouseUpDelay != nil) {
		return ModeDrag
	}
	return ModeExec
}

// TypeLabel is a short type for the list row: drag / exec / pinch / hold.
func (g Gesture) TypeLabel() string {
	switch g.Kind {
	case KindPinch:
		return "pinch"
	case KindHold:
		return "hold"
	case KindSwipe:
		if g.Mode() == ModeDrag {
			return "drag"
		}
		return "exec"
	default:
		return string(g.Kind)
	}
}

// ActionSummary is a one-line description for the list.
func (g Gesture) ActionSummary() string {
	switch g.Kind {
	case KindHold:
		if g.Action != "" {
			return g.Action
		}
		return "(no action)"
	case KindPinch:
		if g.End != "" {
			return g.End
		}
		return cmdTriple(g.Start, g.Update, g.End)
	case KindSwipe:
		if g.Mode() == ModeDrag {
			acc, delay := 20, int64(500)
			if g.Acceleration != nil {
				acc = *g.Acceleration
			}
			if g.MouseUpDelay != nil {
				delay = *g.MouseUpDelay
			}
			return fmt.Sprintf("mouse drag · accel %d · delay %dms", acc, delay)
		}
		if g.End != "" {
			return g.End
		}
		return cmdTriple(g.Start, g.Update, g.End)
	default:
		return ""
	}
}

func cmdTriple(start, update, end string) string {
	parts := make([]string, 0, 3)
	if start != "" {
		parts = append(parts, "start:"+start)
	}
	if update != "" {
		parts = append(parts, "update:"+update)
	}
	if end != "" {
		parts = append(parts, "end:"+end)
	}
	if len(parts) == 0 {
		return "(no command)"
	}
	return strings.Join(parts, " · ")
}

// DisplayName prefers Name, else a derived label.
func (g Gesture) DisplayName() string {
	if strings.TrimSpace(g.Name) != "" {
		return g.Name
	}
	dir := g.Direction
	if dir == "" {
		dir = "-"
	}
	return fmt.Sprintf("%s %df %s", g.TypeLabel(), g.Fingers, dir)
}

// SwipeDirections are valid swipe direction property values.
var SwipeDirections = []string{"any", "n", "s", "e", "w", "ne", "nw", "se", "sw"}

// PinchDirections are valid pinch direction property values.
var PinchDirections = []string{"in", "out", "clockwise", "counterclockwise", "any"}

// NewDragSwipe returns a default 3-finger drag gesture.
func NewDragSwipe() Gesture {
	acc := 20
	delay := int64(500)
	return Gesture{
		Name:         "3-finger drag",
		Kind:         KindSwipe,
		Direction:    "any",
		Fingers:      3,
		Acceleration: &acc,
		MouseUpDelay: &delay,
	}
}

// NewExecSwipe returns a default 4-finger exec swipe.
func NewExecSwipe() Gesture {
	return Gesture{
		Name:      "workspace",
		Kind:      KindSwipe,
		Direction: "w",
		Fingers:   4,
		End:       "xdotool key super+Page_Up",
	}
}
