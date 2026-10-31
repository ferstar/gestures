// Package uiapp is the MyGo native UI control panel state and views.
package uiapp

import (
	"fmt"
	"math"
	"os"
	"strings"

	"github.com/egoist/mygo"

	"gestures-panel/internal/daemon"
	"gestures-panel/internal/kdl"
	"gestures-panel/internal/model"
)

// App is the window state.
type App struct {
	Win *mygo.Window

	Tab int // 0=Gestures, 1=Settings

	ConfigPath string
	Config     *model.Config
	Status     daemon.Status

	StatusMsg string
	StatusErr string

	// Edit drawer/modal
	Editing      bool
	EditIndex    int // -1 = add
	Edit         model.Gesture
	EditMode     model.ActionMode // drag vs exec (for swipe)
	EditKindStr  string
	EditDir      string
	EditFingers  float64
	EditDelay    float64
	EditCommand  string // end or action
	EditArgsText string // optional extra args joined into command
	EditName     string
	EditError    string

	ConfirmDelete bool
	DeleteIndex   int

	// Touchpad card; synced from Config.Touchpad on load.
	TouchProfile  string
	TouchSpeedSet bool
	TouchSpeed    float64
}

// New loads config and status.
func New() *App {
	a := &App{
		ConfigPath:  kdl.ResolvePath(),
		Config:      &model.Config{},
		EditIndex:   -1,
		EditFingers: 3,
		EditDelay:   500,
		EditMode:    model.ModeDrag,
		EditKindStr: string(model.KindSwipe),
		EditDir:     "any",
	}
	a.ReloadConfig()
	a.RefreshStatus()
	return a
}

func (a *App) update(fn func()) {
	if a.Win != nil {
		a.Win.Update(fn)
		return
	}
	fn()
}

// RefreshStatus queries daemon process and autostart state.
func (a *App) RefreshStatus() {
	a.Status = daemon.Refresh()
}

// ReloadConfig reads the KDL file from disk.
func (a *App) ReloadConfig() {
	a.StatusErr = ""
	path := a.ConfigPath
	if path == "" {
		path = kdl.ResolvePath()
		a.ConfigPath = path
	}
	cfg, err := kdl.Load(path)
	if err != nil {
		if os.IsNotExist(err) {
			a.Config = &model.Config{}
			a.StatusMsg = "No config yet — add gestures and Save"
			return
		}
		a.StatusErr = err.Error()
		a.Config = &model.Config{}
		return
	}
	a.Config = cfg
	a.syncTouchpadEdit()
	a.StatusMsg = fmt.Sprintf("Loaded %d gesture(s) from %s", len(cfg.Gestures), path)
}

// SaveConfig writes KDL and reloads the service.
func (a *App) SaveConfig() {
	a.StatusErr = ""
	if a.Config == nil {
		a.Config = &model.Config{}
	}
	if err := kdl.Save(a.ConfigPath, a.Config); err != nil {
		a.StatusErr = "Save failed: " + err.Error()
		return
	}
	a.StatusMsg = "Saved " + a.ConfigPath
	if err := daemon.Reload(); err != nil {
		// Not fatal: daemon may not be started yet
		a.StatusMsg += " (daemon not running)"
	} else {
		a.StatusMsg += " · reloaded"
	}
	a.RefreshStatus()
}

// StartService / StopService / ReloadService manage daemon.
func (a *App) StartService() {
	a.StatusErr = ""
	if err := daemon.Start(); err != nil {
		a.StatusErr = err.Error()
	} else {
		a.StatusMsg = "Daemon started"
	}
	a.RefreshStatus()
}

func (a *App) StopService() {
	a.StatusErr = ""
	if err := daemon.Stop(); err != nil {
		a.StatusErr = err.Error()
	} else {
		a.StatusMsg = "Daemon stopped"
	}
	a.RefreshStatus()
}

func (a *App) ReloadService() {
	a.StatusErr = ""
	if err := daemon.Reload(); err != nil {
		a.StatusErr = err.Error()
	} else {
		a.StatusMsg = "Daemon reloaded"
	}
	a.RefreshStatus()
}

func (a *App) ToggleAutostart() {
	enable := !a.Status.Autostart
	if err := daemon.SetAutostart(enable); err != nil {
		a.StatusErr = err.Error()
	}
	a.RefreshStatus()
}

func (a *App) syncTouchpadEdit() {
	tp := model.Touchpad{}
	if a.Config != nil {
		tp = a.Config.Touchpad
	}
	a.TouchProfile = tp.AccelProfile
	a.TouchSpeedSet = tp.AccelSpeed != nil
	a.TouchSpeed = 0
	if tp.AccelSpeed != nil {
		a.TouchSpeed = *tp.AccelSpeed
	}
}

// ApplyTouchpad writes the touchpad card into the config and saves it.
// An unset field is omitted; the engine then keeps the current device value.
func (a *App) ApplyTouchpad() {
	if a.Config == nil {
		a.Config = &model.Config{}
	}
	tp := model.Touchpad{AccelProfile: a.TouchProfile}
	if a.TouchSpeedSet {
		speed := math.Round(math.Max(-1, math.Min(1, a.TouchSpeed))*100) / 100
		tp.AccelSpeed = &speed
	}
	a.Config.Touchpad = tp
	a.syncTouchpadEdit()
	a.SaveConfig()
}

func (a *App) OpenEditor(index int) {
	a.EditError = ""
	a.EditIndex = index
	if index < 0 || a.Config == nil || index >= len(a.Config.Gestures) {
		g := model.NewDragSwipe()
		a.applyEditFrom(g)
		a.EditIndex = -1
	} else {
		a.applyEditFrom(a.Config.Gestures[index])
	}
	a.Editing = true
}

func (a *App) applyEditFrom(g model.Gesture) {
	a.Edit = g
	a.EditName = g.Name
	a.EditKindStr = string(g.Kind)
	a.EditDir = g.Direction
	if a.EditDir == "" {
		a.EditDir = "any"
	}
	a.EditFingers = float64(g.Fingers)
	if g.Fingers == 0 {
		a.EditFingers = 3
	}
	a.EditMode = g.Mode()
	if g.MouseUpDelay != nil {
		a.EditDelay = float64(*g.MouseUpDelay)
	} else {
		a.EditDelay = 500
	}
	switch g.Kind {
	case model.KindHold:
		a.EditCommand = g.Action
	default:
		a.EditCommand = g.End
	}
	a.EditArgsText = ""
}

func (a *App) CommitEdit() {
	a.EditError = ""
	g, err := a.buildGestureFromEdit()
	if err != nil {
		a.EditError = err.Error()
		return
	}
	if a.Config == nil {
		a.Config = &model.Config{}
	}
	if a.EditIndex < 0 {
		a.Config.Gestures = append(a.Config.Gestures, g)
	} else if a.EditIndex < len(a.Config.Gestures) {
		a.Config.Gestures[a.EditIndex] = g
	} else {
		a.Config.Gestures = append(a.Config.Gestures, g)
	}
	a.Editing = false
	a.SaveConfig()
}

func (a *App) buildGestureFromEdit() (model.Gesture, error) {
	kind := model.Kind(a.EditKindStr)
	fingers := int(a.EditFingers)
	if fingers < 1 || fingers > 10 {
		return model.Gesture{}, fmt.Errorf("fingers must be 1–10")
	}
	g := model.Gesture{
		Name:    strings.TrimSpace(a.EditName),
		Kind:    kind,
		Fingers: fingers,
	}
	cmd := strings.TrimSpace(a.EditCommand)
	if extra := strings.TrimSpace(a.EditArgsText); extra != "" {
		// Allow "args list" UI: append space-separated tokens.
		cmd = strings.TrimSpace(cmd + " " + extra)
	}
	switch kind {
	case model.KindSwipe:
		g.Direction = strings.ToLower(strings.TrimSpace(a.EditDir))
		if g.Direction == "" {
			return g, fmt.Errorf("direction required")
		}
		if a.EditMode == model.ModeDrag {
			delay := int64(a.EditDelay)
			g.MouseUpDelay = &delay
		} else {
			if cmd == "" {
				return g, fmt.Errorf("command required for exec")
			}
			g.End = cmd
		}
	case model.KindPinch:
		g.Direction = strings.ToLower(strings.TrimSpace(a.EditDir))
		if g.Direction == "" {
			return g, fmt.Errorf("direction required")
		}
		if cmd == "" {
			return g, fmt.Errorf("command required")
		}
		g.End = cmd
	case model.KindHold:
		if cmd == "" {
			return g, fmt.Errorf("action required")
		}
		g.Action = cmd
	default:
		return g, fmt.Errorf("unknown kind %q", a.EditKindStr)
	}
	return g, nil
}

func (a *App) DeleteAt(index int) {
	if a.Config == nil || index < 0 || index >= len(a.Config.Gestures) {
		return
	}
	a.Config.Gestures = append(a.Config.Gestures[:index], a.Config.Gestures[index+1:]...)
	a.ConfirmDelete = false
	a.SaveConfig()
}

// BuildCommand joins program + args the way the blog example showed:
// exec "xdotool" "key" "super+Page_Up" → "xdotool key super+Page_Up"
func BuildCommand(prog string, args []string) string {
	parts := make([]string, 0, 1+len(args))
	prog = strings.TrimSpace(prog)
	if prog != "" {
		parts = append(parts, prog)
	}
	for _, a := range args {
		a = strings.TrimSpace(a)
		if a != "" {
			parts = append(parts, a)
		}
	}
	return strings.Join(parts, " ")
}
