package uiapp

import (
	"image/png"
	"os"
	"strings"
	"testing"

	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/daemon"
	"gestures-panel/internal/kdl"
	"gestures-panel/internal/model"
)

func TestZZShots(t *testing.T) {
	dir := os.Getenv("SHOT_DIR")
	if dir == "" {
		t.Skip()
	}
	cfg, err := kdl.Load("../../testdata/sample.kdl")
	if err != nil {
		t.Fatal(err)
	}
	speed := 0.3
	cfg.Touchpad = model.Touchpad{AccelProfile: "adaptive", AccelSpeed: &speed}
	mk := func() *App {
		a := &App{ConfigPath: "/home/user/.config/gestures.kdl", Config: cfg, EditIndex: -1,
			Status:    daemon.Status{Active: true, Autostart: true, DisplayServer: "wayland", InputGroupOK: true, InInputGroup: true},
			StatusMsg: "Loaded 6 gesture(s)"}
		a.syncTouchpadEdit()
		return a
	}
	width, height := 900, 680
	shot := func(name string, a *App, dark bool) {
		tt := ui.NewTester(a.View, width, height)
		tt.SetDark(dark)
		tt.Frame()
		f, _ := os.Create(dir + "/" + name + ".png")
		defer f.Close()
		png.Encode(f, tt.Image())
	}
	a := mk()
	a.Tab = 2
	shot("service", a, false)
	shot("service-dark", a, true)
	a = mk()
	a.Tab = 0
	shot("gestures", a, false)
	shot("gestures-dark", a, true)
	a.Tab = 1
	shot("touchpad", a, false)
	shot("touchpad-dark", a, true)
	a.Tab = 0
	a.OpenEditor(0)
	shot("edit", a, false)
	shot("edit-dark", a, true)
	width, height = 720, 560
	a.Editing = false
	shot("gestures-default", a, false)
	a.Tab = 1
	shot("touchpad-default", a, false)
	a.Tab = 2
	shot("service-default", a, false)
	a.Tab = 0
	a.OpenEditor(1)
	shot("edit-command-default", a, false)
	a.Editing = false
	cfg.Gestures[0].End = strings.Repeat("very-long-command ", 20)
	cfg.Gestures[0].MouseUpDelay = nil
	shot("long-command-default", a, false)
	a.Config = &model.Config{}
	a.Status.Active = false
	shot("empty-default", a, false)

	// Keep every direction visible together to inspect glyph fallback and alignment.
	width, height = 720, 850
	a = mk()
	a.Config = &model.Config{Gestures: []model.Gesture{
		{Kind: model.KindPinch, Fingers: 2, Direction: "in", End: "xdotool key ctrl+minus"},
		{Kind: model.KindPinch, Fingers: 2, Direction: "out", End: "xdotool key ctrl+plus"},
		{Kind: model.KindPinch, Fingers: 2, Direction: "clockwise", End: "rotate clockwise"},
		{Kind: model.KindPinch, Fingers: 2, Direction: "counterclockwise", End: "rotate counterclockwise"},
		{Kind: model.KindPinch, Fingers: 2, Direction: "any", End: "pinch action"},
		{Kind: model.KindHold, Fingers: 2, Action: "rofi -show drun"},
		{Kind: model.KindSwipe, Fingers: 2, Direction: "nw", End: "workspace up-left"},
	}}
	shot("directions", a, false)
	shot("directions-dark", a, true)
}
