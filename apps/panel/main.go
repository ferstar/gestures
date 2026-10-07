package main

import (
	_ "embed"
	"log"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/systemd"
	"gestures-panel/internal/uiapp"
)

//go:embed resources/tray.png
var trayIcon []byte

func main() {
	a := uiapp.New()
	mygo.App.SetName("Gestures Panel")

	var (
		mainWin  *mygo.Window
		tray     *mygo.Tray
		trayMenu *mygo.Menu
	)

	openPanel := func() {
		if mainWin != nil && !mainWin.IsDestroyed() {
			mainWin.Show()
			mainWin.Focus()
			return
		}
		mainWin = mygo.NewWindow(mygo.WindowOptions{
			Title:     "Gestures Panel",
			Width:     720,
			Height:    560,
			MinWidth:  480,
			MinHeight: 400,
			StateKey:  "main",
			Content:   ui.View(a.View),
		})
		a.Win = mainWin
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
			{ID: "status", Label: "Status: " + label, Disabled: true},
			mygo.Separator(),
			{Label: "Open panel", Click: func(*mygo.MenuItem, *mygo.Window) {
				openPanel()
			}},
			{Label: "Reload config", Click: func(*mygo.MenuItem, *mygo.Window) {
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
		tray.SetToolTip("Gestures Panel — " + label)
	}

	mygo.App.WhenReady(func() {
		openPanel()

		a.RefreshStatus()
		label := statusLabelFrom(a)
		trayMenu = mygo.NewMenu([]*mygo.MenuItem{
			{ID: "status", Label: "Status: " + label, Disabled: true},
			mygo.Separator(),
			{Label: "Open panel", Click: func(*mygo.MenuItem, *mygo.Window) {
				openPanel()
			}},
			{Label: "Reload config", Click: func(*mygo.MenuItem, *mygo.Window) {
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
			ToolTip: "Gestures Panel — " + label,
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
		return "Running"
	}
	return "Stopped"
}
