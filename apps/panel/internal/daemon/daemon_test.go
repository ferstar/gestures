package daemon_test

import (
	"testing"

	"gestures-panel/internal/daemon"
)

func TestDaemonSmoke(t *testing.T) {
	st := daemon.Refresh()
	if st.DisplayServer == "" {
		t.Fatal("expected display server")
	}
	if st.SocketPath == "" {
		t.Fatal("expected socket path")
	}
}
