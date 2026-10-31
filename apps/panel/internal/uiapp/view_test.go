package uiapp

import (
	"testing"

	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/model"
)

// Since mygo 0.3, bound input is applied after the build, so the derived
// action-mode segment must commit its edit back to the model.
func TestEditModalActionTypeSegmentCommits(t *testing.T) {
	a := &App{
		Config:      &model.Config{},
		Editing:     true,
		EditIndex:   -1,
		EditKindStr: string(model.KindSwipe),
		EditMode:    model.ModeDrag,
		EditDir:     "any",
		EditFingers: 3,
		EditDelay:   500,
	}
	tt := ui.NewTester(a.View, 720, 560)
	tt.Frame()
	if !tt.HasText("Mouse-up delay (ms)") {
		t.Fatalf("drag fields missing, texts=%q", tt.Texts())
	}
	if err := tt.Click("Command"); err != nil {
		t.Fatalf("choose command: %v, texts=%q", err, tt.Texts())
	}
	tt.Frame()
	if a.EditMode != model.ModeExec {
		t.Fatalf("EditMode = %q, want %q", a.EditMode, model.ModeExec)
	}
	if !tt.HasText("Command") || tt.HasText("Mouse-up delay (ms)") {
		t.Fatalf("exec fields not shown, texts=%q", tt.Texts())
	}
	if err := tt.Click("Drag"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.EditMode != model.ModeDrag || !tt.HasText("Mouse-up delay (ms)") || tt.HasText("Extra args") {
		t.Fatalf("drag fields not restored, texts=%q", tt.Texts())
	}
	if err := tt.Click("pinch"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.EditKindStr != "pinch" || a.EditDir != "in" || !tt.HasText("Command") {
		t.Fatalf("pinch fields invalid: kind=%s dir=%s texts=%q", a.EditKindStr, a.EditDir, tt.Texts())
	}
	if err := tt.Click("hold"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.EditKindStr != "hold" || tt.HasText("Direction") || !tt.HasText("Action") {
		t.Fatalf("hold fields invalid, texts=%q", tt.Texts())
	}
}

func TestTouchpadTabShowsTouchpadCard(t *testing.T) {
	a := &App{Tab: 1, Config: &model.Config{}, TouchSpeedSet: true, TouchSpeed: 0.3}
	tt := ui.NewTester(a.View, 720, 560)
	tt.Frame()
	if !tt.HasText("Touchpad") || !tt.HasText("Accel profile") || !tt.HasText("0.30") {
		t.Fatalf("touchpad card missing, texts=%q", tt.Texts())
	}
}

func TestTouchpadProfileSegments(t *testing.T) {
	a := &App{Tab: 1, Config: &model.Config{}, TouchProfile: "adaptive"}
	tt := ui.NewTester(a.View, 720, 560)
	tt.Frame()
	for _, choice := range []struct{ label, value string }{{"Flat", "flat"}, {"Default", ""}, {"Adaptive", "adaptive"}} {
		if err := tt.Click(choice.label); err != nil {
			t.Fatal(err)
		}
		tt.Frame()
		if a.TouchProfile != choice.value {
			t.Fatalf("%s profile = %q", choice.label, a.TouchProfile)
		}
	}
}

func TestDefaultTabGroupsWithoutReorderingConfig(t *testing.T) {
	a := &App{Config: &model.Config{Gestures: []model.Gesture{
		{Name: "Four fingers", Kind: model.KindSwipe, Fingers: 4, Direction: "w", End: "workspace left"},
		{Name: "Two fingers", Kind: model.KindPinch, Fingers: 2, Direction: "in", End: "zoom out"},
	}}}
	tt := ui.NewTester(a.View, 720, 560)
	tt.Frame()
	if !tt.HasText("2 FINGERS") || !tt.HasText("4 FINGERS") || !tt.HasText("zoom out") || tt.HasText("Accel profile") {
		t.Fatalf("default gesture list invalid, texts=%q", tt.Texts())
	}
	if err := tt.Click("Two fingers"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if !a.Editing || a.EditIndex != 1 || a.EditName != "Two fingers" {
		t.Fatalf("grouped row opened wrong gesture: editing=%v index=%d name=%q", a.Editing, a.EditIndex, a.EditName)
	}
	if a.Config.Gestures[0].Name != "Four fingers" {
		t.Fatal("display grouping reordered the stored config")
	}
}

func TestUnnamedGestureTitlesStayInPresentation(t *testing.T) {
	drag := model.NewDragSwipe()
	drag.Name = ""
	gestures := []model.Gesture{
		{Kind: model.KindPinch, Fingers: 2, Direction: "in", End: "zoom in"},
		{Kind: model.KindPinch, Fingers: 2, Direction: "out", End: "zoom out"},
		{Kind: model.KindPinch, Fingers: 2, Direction: "clockwise", End: "rotate"},
		{Kind: model.KindSwipe, Fingers: 2, Direction: "nw", End: "workspace"},
		{Kind: model.KindHold, Fingers: 2, Action: "launcher"},
		drag,
		{Name: "Custom title", Kind: model.KindPinch, Fingers: 2, Direction: "in", End: "custom"},
	}
	a := &App{Config: &model.Config{Gestures: gestures}}
	tt := ui.NewTester(a.View, 900, 900)
	tt.Frame()
	for _, title := range []string{"Pinch in", "Pinch out", "Rotate clockwise", "Swipe up-left", "Hold", "Drag", "Custom title"} {
		if !tt.HasText(title) {
			t.Fatalf("missing title %q, texts=%q", title, tt.Texts())
		}
	}
	for i := 0; i < len(gestures)-1; i++ {
		a.OpenEditor(i)
		if a.Config.Gestures[i].Name != "" || a.EditName != "" {
			t.Fatalf("gesture %d acquired a persisted display name", i)
		}
	}
}
