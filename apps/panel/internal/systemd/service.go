// Package systemd talks to the gestures user systemd unit and local env checks.
package systemd

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"os/user"
	"runtime"
	"strings"
)

const Unit = "gestures.service"

// Status describes the user service and environment.
type Status struct {
	Available     bool   // systemctl --user works
	Active        bool
	Enabled       bool
	State         string // active/inactive/failed/...
	DisplayServer string // Wayland / X11 / unknown
	InInputGroup  bool
	InputGroupOK  bool // whether we could check
	Message       string
	Error         string
}

// Refresh queries systemd and environment. Safe on non-Linux (returns stubs).
func Refresh() Status {
	st := Status{
		DisplayServer: DetectDisplayServer(),
	}
	st.InInputGroup, st.InputGroupOK = InInputGroup()

	if runtime.GOOS != "linux" {
		st.Message = "systemd user service controls require Linux"
		return st
	}
	if _, err := exec.LookPath("systemctl"); err != nil {
		st.Message = "systemctl not found"
		st.Error = err.Error()
		return st
	}

	out, err := run("systemctl", "--user", "is-active", Unit)
	st.Available = true
	if err != nil {
		// is-active returns non-zero when inactive
		st.State = strings.TrimSpace(out)
		if st.State == "" {
			st.State = "inactive"
		}
	} else {
		st.State = strings.TrimSpace(out)
	}
	st.Active = st.State == "active"

	enOut, enErr := run("systemctl", "--user", "is-enabled", Unit)
	en := strings.TrimSpace(enOut)
	st.Enabled = enErr == nil && (en == "enabled" || en == "enabled-runtime" || en == "static")
	if en != "" && st.State == "" {
		st.State = en
	}
	return st
}

// Start enables-and-starts or just starts the unit.
func Start() error {
	return runErr("systemctl", "--user", "start", Unit)
}

// Stop stops the unit.
func Stop() error {
	return runErr("systemctl", "--user", "stop", Unit)
}

// Restart restarts the unit.
func Restart() error {
	return runErr("systemctl", "--user", "restart", Unit)
}

// Reload asks systemd to reload the unit (ExecReload → gestures reload).
func Reload() error {
	// Prefer systemctl reload; fall back to `gestures reload` if unit missing.
	if err := runErr("systemctl", "--user", "reload", Unit); err == nil {
		return nil
	}
	if _, lookErr := exec.LookPath("gestures"); lookErr == nil {
		return runErr("gestures", "reload")
	}
	return fmt.Errorf("could not reload: systemctl --user reload %s failed and gestures binary not found", Unit)
}

// EnableNow enables and starts.
func EnableNow() error {
	return runErr("systemctl", "--user", "enable", "--now", Unit)
}

// DetectDisplayServer mirrors ferstar/gestures detect_wayland().
func DetectDisplayServer() string {
	if os.Getenv("WAYLAND_DISPLAY") != "" {
		return "Wayland"
	}
	if t := strings.ToLower(os.Getenv("XDG_SESSION_TYPE")); t != "" {
		switch t {
		case "wayland":
			return "Wayland"
		case "x11":
			return "X11"
		default:
			return t
		}
	}
	if os.Getenv("DISPLAY") != "" {
		return "X11"
	}
	return "unknown"
}

// InInputGroup reports whether the current user is in the input group.
func InInputGroup() (in bool, ok bool) {
	u, err := user.Current()
	if err != nil {
		return false, false
	}
	gids, err := u.GroupIds()
	if err != nil {
		// Fallback: parse `id -nG`
		out, err2 := run("id", "-nG")
		if err2 != nil {
			return false, false
		}
		for _, g := range strings.Fields(out) {
			if g == "input" {
				return true, true
			}
		}
		return false, true
	}
	for _, gid := range gids {
		g, err := user.LookupGroupId(gid)
		if err != nil {
			continue
		}
		if g.Name == "input" {
			return true, true
		}
	}
	return false, true
}

func run(name string, args ...string) (string, error) {
	cmd := exec.Command(name, args...)
	var buf bytes.Buffer
	cmd.Stdout = &buf
	cmd.Stderr = &buf
	err := cmd.Run()
	return buf.String(), err
}

func runErr(name string, args ...string) error {
	out, err := run(name, args...)
	if err != nil {
		msg := strings.TrimSpace(out)
		if msg == "" {
			return err
		}
		return fmt.Errorf("%s: %s", err, msg)
	}
	return nil
}
