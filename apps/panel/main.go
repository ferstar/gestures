package main

import (
	_ "embed"
	"fmt"
	"log"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/i18n"
	"gestures-panel/internal/systemd"
	"gestures-panel/internal/uiapp"
)

//go:embed resources/tray.png
var trayIcon []byte

// trayMenu is package-level on purpose: the Linux tray only weakly references
// menu items, so a local menu can be garbage collected and its clicks lost.
var trayMenu *mygo.Menu

func main() {
	a := uiapp.New()
	mygo.App.SetName(i18n.T("window_title"))

	var (
		mainWin  *mygo.Window
		tray     *mygo.Tray
		quitting bool
	)

	mygo.App.OnBeforeQuit(func(*mygo.QuitEvent) { quitting = true })

	openPanel := func() {
		if mainWin != nil && !mainWin.IsDestroyed() {
			mainWin.Show()
			mainWin.Focus()
			return
		}
		mainWin = mygo.NewWindow(mygo.WindowOptions{
			Title:     i18n.T("window_title"),
			Width:     720,
			Height:    560,
			MinWidth:  480,
			MinHeight: 400,
			StateKey:  "main",
			Content:   ui.View(a.View),
		})
		a.Win = mainWin
		// Closing hides the window instead of destroying it, so "Open panel"
		// in the tray always reuses it. A real quit still closes it.
		mainWin.OnClose(func(e *mygo.CloseEvent) {
			if !quitting {
				e.PreventDefault()
				mainWin.Hide()
			}
		})
		mainWin.OnClosed(func() {
			mainWin = nil
			a.Win = nil
		})
	}

	var refreshTray func()
	refreshTray = func() {
		if tray == nil {
			return
		}
		a.RefreshStatus()
		label := statusLabelFrom(a)
		trayMenu = mygo.NewMenu([]*mygo.MenuItem{
			{ID: "status", Label: fmt.Sprintf(i18n.T("tray_status"), label), Disabled: true},
			mygo.Separator(),
			{Label: i18n.T("tray_open"), Click: func(*mygo.MenuItem, *mygo.Window) {
				openPanel()
			}},
			{Label: i18n.T("tray_reload"), Click: func(*mygo.MenuItem, *mygo.Window) {
				a.ReloadConfig()
				_ = systemd.Reload()
				a.RefreshStatus()
				if a.Win != nil {
					a.Win.Update(func() {})
				}
				refreshTray()
			}},
			mygo.Separator(),
			{Role: mygo.RoleQuit},
		})
		tray.SetMenu(trayMenu)
		tray.SetToolTip(i18n.T("window_title") + " — " + label)
	}

	mygo.App.WhenReady(func() {
		openPanel()

		a.RefreshStatus()
		label := statusLabelFrom(a)
		trayMenu = mygo.NewMenu([]*mygo.MenuItem{
			{ID: "status", Label: fmt.Sprintf(i18n.T("tray_status"), label), Disabled: true},
			mygo.Separator(),
			{Label: i18n.T("tray_open"), Click: func(*mygo.MenuItem, *mygo.Window) {
				openPanel()
			}},
			{Label: i18n.T("tray_reload"), Click: func(*mygo.MenuItem, *mygo.Window) {
				a.ReloadConfig()
				_ = systemd.Reload()
				a.RefreshStatus()
				if a.Win != nil {
					a.Win.Update(func() {})
				}
				refreshTray()
			}},
			mygo.Separator(),
			{Role: mygo.RoleQuit},
		})
		var err error
		tray, err = mygo.NewTray(mygo.TrayOptions{
			Icon:    trayIcon,
			ToolTip: i18n.T("window_title") + " — " + label,
			Menu:    trayMenu,
		})
		if err != nil {
			log.Println("tray unavailable (need libayatana-appindicator3 on Linux):", err)
			tray = nil
		}
	})

	// Stay alive when the window is closed so the tray keeps working.
	mygo.App.OnWindowAllClosed(func() {})
	mygo.App.OnActivate(func(hasVisibleWindows bool) {
		if !hasVisibleWindows {
			openPanel()
		}
	})

	if err := mygo.App.Run(); err != nil {
		log.Fatal(err)
	}
}

func statusLabelFrom(a *uiapp.App) string {
	if a.Status.Active {
		return i18n.T("running")
	}
	return i18n.T("stopped")
}
