package kdl

import (
	"os"
	"strings"
	"testing"

	"gestures-panel/internal/model"
)

func TestParseDefaultSample(t *testing.T) {
	src := `// Gestures Configuration
// See https://github.com/ferstar/gestures for full documentation

swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20

// Hyprland:
// swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"

swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"
swipe direction="e" fingers=4 end="hyprctl dispatch workspace e+1"

pinch direction="in" fingers=2 end="xdotool key ctrl+minus"
hold fingers=4 action="rofi -show drun"
`
	cfg, err := Parse(src)
	if err != nil {
		t.Fatal(err)
	}
	if len(cfg.Gestures) != 5 {
		t.Fatalf("got %d gestures, want 5", len(cfg.Gestures))
	}
	g0 := cfg.Gestures[0]
	if g0.Kind != model.KindSwipe || g0.Direction != "any" || g0.Fingers != 3 {
		t.Fatalf("g0: %+v", g0)
	}
	if g0.Acceleration == nil || *g0.Acceleration != 20 {
		t.Fatalf("accel: %+v", g0.Acceleration)
	}
	if g0.MouseUpDelay == nil || *g0.MouseUpDelay != 500 {
		t.Fatalf("delay: %+v", g0.MouseUpDelay)
	}
	if g0.TypeLabel() != "drag" {
		t.Fatalf("type label: %s", g0.TypeLabel())
	}
	g1 := cfg.Gestures[1]
	if g1.End != "hyprctl dispatch workspace e-1" {
		t.Fatalf("end: %q", g1.End)
	}
	if cfg.Gestures[3].Kind != model.KindPinch {
		t.Fatalf("pinch: %+v", cfg.Gestures[3])
	}
	if cfg.Gestures[4].Action != "rofi -show drun" {
		t.Fatalf("hold: %+v", cfg.Gestures[4])
	}
}

func TestRoundTrip(t *testing.T) {
	src := `swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20
swipe direction="w" fingers=4 end="xdotool key super+Page_Up"
pinch direction="out" fingers=2 end="xdotool key ctrl+plus"
hold fingers=3 action="flameshot gui"
`
	cfg, err := Parse(src)
	if err != nil {
		t.Fatal(err)
	}
	cfg.Gestures[0].Name = "3-finger drag"
	out := Format(cfg)
	cfg2, err := Parse(out)
	if err != nil {
		t.Fatal(err)
	}
	if len(cfg2.Gestures) != 4 {
		t.Fatalf("round-trip count %d", len(cfg2.Gestures))
	}
	if cfg2.Gestures[0].Name != "3-finger drag" {
		t.Fatalf("name lost: %q", cfg2.Gestures[0].Name)
	}
	if cfg2.Gestures[1].End != "xdotool key super+Page_Up" {
		t.Fatalf("end: %q", cfg2.Gestures[1].End)
	}
	if !strings.Contains(out, `mouse-up-delay=500`) {
		t.Fatalf("missing delay in:\n%s", out)
	}
}

func TestLineContinuation(t *testing.T) {
	src := `swipe direction="any" fingers=3 \
  start="ydotool click -- 0x40" \
  update="ydotool mousemove -x $delta_x -y $delta_y" \
  end="ydotool click -- 0x80"
`
	cfg, err := Parse(src)
	if err != nil {
		t.Fatal(err)
	}
	if len(cfg.Gestures) != 1 {
		t.Fatalf("count %d", len(cfg.Gestures))
	}
	g := cfg.Gestures[0]
	if g.Start == "" || g.Update == "" || g.End == "" {
		t.Fatalf("%+v", g)
	}
}

func TestBlogExampleIgnoredShape(t *testing.T) {
	// The blog used a different shape; we intentionally support the real
	// ferstar/gestures schema only. Unknown nodes should error.
	_, err := Parse(`gesture "drag" swipe any { fingers 3 }`)
	if err == nil {
		t.Fatal("expected error for blog-shaped KDL")
	}
}

func TestParseRealDefaultConfig(t *testing.T) {
	// Active (uncommented) nodes only — default file has mostly comments.
	src := `swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20
`
	cfg, err := Parse(src)
	if err != nil {
		t.Fatal(err)
	}
	if len(cfg.Gestures) != 1 {
		t.Fatalf("%d", len(cfg.Gestures))
	}
}

func TestParseTestdataSample(t *testing.T) {
	cfg, err := Load("../../testdata/sample.kdl")
	if err != nil {
		t.Fatal(err)
	}
	if len(cfg.Gestures) != 6 {
		t.Fatalf("got %d", len(cfg.Gestures))
	}
	if cfg.Gestures[0].Name != "3-finger drag" {
		t.Fatalf("name %q", cfg.Gestures[0].Name)
	}
}

func TestParseFerstarDefaultFile(t *testing.T) {
	data, err := os.ReadFile("/tmp/default-gestures.kdl")
	if err != nil {
		t.Skip(err)
	}
	cfg, err := Parse(string(data))
	if err != nil {
		t.Fatal(err)
	}
	// Default shipped config has one active swipe; rest are comments.
	if len(cfg.Gestures) != 1 {
		t.Fatalf("got %d gestures", len(cfg.Gestures))
	}
	if cfg.Gestures[0].TypeLabel() != "drag" {
		t.Fatalf("want drag, got %s", cfg.Gestures[0].TypeLabel())
	}
}
