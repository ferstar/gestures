// Package daemon manages the gestures background process, IPC communication, and autostart configuration.
package daemon

import (
	"bytes"
	"fmt"
	"net"
	"os"
	"os/exec"
	"os/user"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"syscall"
	"time"
)

// Status describes the background daemon process and environment.
type Status struct {
	Active        bool   // process is running and/or socket responsive
	Pid           int    // process pid if running
	Autostart     bool   // ~/.config/autostart/gestures.desktop exists and enabled
	DisplayServer string // Wayland / X11 / unknown
	InInputGroup  bool
	InputGroupOK  bool
	SocketPath    string
	Message       string
	Error         string
}

// SocketPath returns the standard IPC socket path for the current user.
func SocketPath() string {
	if dir := os.Getenv("XDG_RUNTIME_DIR"); dir != "" {
		return filepath.Join(dir, "gestures.sock")
	}
	uid := os.Getuid()
	fallback := fmt.Sprintf("/run/user/%d/gestures.sock", uid)
	return fallback
}

// AutostartFilePath returns ~/.config/autostart/gestures.desktop.
func AutostartFilePath() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, ".config", "autostart", "gestures.desktop")
}

// IsAutostartEnabled checks if autostart desktop entry exists and is active.
func IsAutostartEnabled() bool {
	p := AutostartFilePath()
	if p == "" {
		return false
	}
	data, err := os.ReadFile(p)
	if err != nil {
		return false
	}
	content := string(data)
	if strings.Contains(content, "Hidden=true") || strings.Contains(content, "X-GNOME-Autostart-enabled=false") {
		return false
	}
	return true
}

// SetAutostart toggles the autostart desktop entry in user's home.
func SetAutostart(enable bool) error {
	p := AutostartFilePath()
	if p == "" {
		return fmt.Errorf("cannot resolve home directory")
	}
	if !enable {
		if err := os.Remove(p); err != nil && !os.IsNotExist(err) {
			return err
		}
		return nil
	}

	if err := os.MkdirAll(filepath.Dir(p), 0755); err != nil {
		return err
	}

	execPath, err := exec.LookPath("gestures")
	if err != nil {
		execPath = "/usr/bin/gestures"
	}

	content := fmt.Sprintf(`[Desktop Entry]
Type=Application
Name=Gestures Daemon
Comment=Touchpad gestures daemon
Exec=%s start
Terminal=false
StartupNotify=false
X-GNOME-Autostart-enabled=true
`, execPath)

	return os.WriteFile(p, []byte(content), 0644)
}

// Refresh checks daemon process state and environment.
func Refresh() Status {
	st := Status{
		DisplayServer: DetectDisplayServer(),
		SocketPath:    SocketPath(),
		Autostart:     IsAutostartEnabled(),
	}
	st.InInputGroup, st.InputGroupOK = InInputGroup()

	// Check if socket is open and listening
	if c, err := net.DialTimeout("unix", st.SocketPath, 150*time.Millisecond); err == nil {
		_ = c.Close()
		st.Active = true
	}

	// Double check via pgrep
	pid := findGesturesPid()
	if pid > 0 {
		st.Pid = pid
		st.Active = true
	}

	return st
}

func findGesturesPid() int {
	out, err := exec.Command("pgrep", "-x", "gestures").Output()
	if err != nil {
		return 0
	}
	lines := strings.Split(strings.TrimSpace(string(out)), "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		if p, err := strconv.Atoi(line); err == nil && p > 0 {
			// Ensure it's not the current process
			if p != os.Getpid() {
				return p
			}
		}
	}
	return 0
}

// Start launches `gestures start` as a detached daemon process.
func Start() error {
	if Refresh().Active {
		return nil
	}

	execPath, err := exec.LookPath("gestures")
	if err != nil {
		execPath = "/usr/bin/gestures"
	}

	cmd := exec.Command(execPath, "start")
	cmd.SysProcAttr = &syscall.SysProcAttr{
		Setsid: true, // Create a new session, completely detaching from the GUI parent
	}
	// Redirect stdio to devnull so process doesn't hold gui pipes
	devNull, err := os.OpenFile(os.DevNull, os.O_RDWR, 0)
	if err == nil {
		cmd.Stdin = devNull
		cmd.Stdout = devNull
		cmd.Stderr = devNull
	}

	if err := cmd.Start(); err != nil {
		if devNull != nil {
			_ = devNull.Close()
		}
		return fmt.Errorf("failed to start gestures: %w", err)
	}

	// Reaping goroutine: waits for the child process so it never becomes a zombie (<defunct>)
	go func() {
		_ = cmd.Wait()
		if devNull != nil {
			_ = devNull.Close()
		}
	}()

	// Give it a moment to bind socket
	for i := 0; i < 10; i++ {
		time.Sleep(50 * time.Millisecond)
		if Refresh().Active {
			break
		}
	}
	return nil
}

// Stop terminates any running gestures process.
func Stop() error {
	pid := findGesturesPid()
	if pid <= 0 {
		return nil
	}
	proc, err := os.FindProcess(pid)
	if err != nil {
		return err
	}
	_ = proc.Signal(syscall.SIGTERM)

	// Wait up to 1 second
	for i := 0; i < 10; i++ {
		time.Sleep(100 * time.Millisecond)
		if findGesturesPid() == 0 {
			break
		}
	}
	if findGesturesPid() > 0 {
		_ = proc.Kill()
	}

	// Clean socket if left over
	_ = os.Remove(SocketPath())
	return nil
}

// Reload sends reload command via `gestures reload` or direct IPC.
func Reload() error {
	execPath, err := exec.LookPath("gestures")
	if err != nil {
		execPath = "/usr/bin/gestures"
	}
	out, err := exec.Command(execPath, "reload").CombinedOutput()
	if err != nil {
		return fmt.Errorf("reload failed: %s (%w)", strings.TrimSpace(string(out)), err)
	}
	return nil
}

// DetectDisplayServer identifies Wayland or X11.
func DetectDisplayServer() string {
	if os.Getenv("WAYLAND_DISPLAY") != "" {
		return "Wayland"
	}
	st := strings.ToLower(os.Getenv("XDG_SESSION_TYPE"))
	switch st {
	case "wayland":
		return "Wayland"
	case "x11":
		return "X11"
	}
	if os.Getenv("DISPLAY") != "" {
		return "X11"
	}
	return "unknown"
}

// InInputGroup checks if current user belongs to the input group.
func InInputGroup() (inGroup bool, ok bool) {
	if runtime.GOOS != "linux" {
		return false, false
	}
	u, err := user.Current()
	if err != nil {
		return false, false
	}
	gids, err := u.GroupIds()
	if err != nil {
		// Fallback to groups command
		out, err := exec.Command("id", "-Gn").Output()
		if err != nil {
			return false, false
		}
		for _, name := range bytes.Fields(out) {
			if string(name) == "input" {
				return true, true
			}
		}
		return false, true
	}
	for _, gid := range gids {
		grp, err := user.LookupGroupId(gid)
		if err == nil && grp.Name == "input" {
			return true, true
		}
	}
	return false, true
}
