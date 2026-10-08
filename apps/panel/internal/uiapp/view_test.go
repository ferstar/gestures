package uiapp

import (
	"testing"

	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/model"
)

// Since mygo 0.3, bound input is applied after the build, so the derived
// action-mode select must commit its edit back to the model.
func TestEditModalActionTypeSelectCommits(t *testing.T) {
	a := &App{
		Config:      &model.Config{},
		Editing:     true,
		EditIndex:   -1,
		EditKindStr: string(model.KindSwipe),
		EditMode:    model.ModeDrag,
		EditDir:     "any",
		EditFingers: 3,
		EditAccel:   20,
		EditDelay:   500,
	}
	tt := ui.NewTester(a.View, 720, 560)
	tt.Frame()
	if !tt.HasText("Acceleration") {
		t.Fatalf("drag fields missing, texts=%q", tt.Texts())
	}
	if err := tt.Click(string(model.ModeDrag)); err != nil {
		t.Fatal(err)
	}
	if err := tt.Click(string(model.ModeExec)); err != nil {
		t.Fatalf("choose exec: %v, texts=%q", err, tt.Texts())
	}
	tt.Frame()
	if a.EditMode != model.ModeExec {
		t.Fatalf("EditMode = %q, want %q", a.EditMode, model.ModeExec)
	}
	if !tt.HasText("Command") || tt.HasText("Acceleration") {
		t.Fatalf("exec fields not shown, texts=%q", tt.Texts())
	}
}
