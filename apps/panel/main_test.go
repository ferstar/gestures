package main

import (
	"testing"

	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/kdl"
	"gestures-panel/internal/uiapp"
)

func TestViewRenders(t *testing.T) {
	a := uiapp.New()
	tt := ui.NewTester(a.View, 720, 560)
	tt.Frame()
	if !tt.HasText("Gestures Panel") {
		t.Fatalf("missing title, texts=%q", tt.Texts())
	}
	if !tt.HasText("Service") {
		t.Fatalf("missing Service tab, texts=%q", tt.Texts())
	}
}

func TestKDLRoundTripInPanel(t *testing.T) {
	src := `swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20
swipe direction="n" fingers=4 end="xdotool key super+Page_Up"
`
	cfg, err := kdl.Parse(src)
	if err != nil {
		t.Fatal(err)
	}
	out := kdl.Format(cfg)
	cfg2, err := kdl.Parse(out)
	if err != nil {
		t.Fatal(err)
	}
	if len(cfg2.Gestures) != 2 {
		t.Fatalf("got %d", len(cfg2.Gestures))
	}
}
