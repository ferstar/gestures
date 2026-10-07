package systemd

import "testing"

func TestRefreshSmoke(t *testing.T) {
	st := Refresh()
	t.Logf("display=%s active=%v available=%v input=%v/%v msg=%q",
		st.DisplayServer, st.Active, st.Available, st.InInputGroup, st.InputGroupOK, st.Message)
	if st.DisplayServer == "" {
		t.Fatal("empty display server")
	}
}
