package main

import (
	_ "embed"
	"log"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/i18n"
	"gestures-panel/internal/uiapp"
)

//go:embed resources/icon.png
var appIcon []byte

func main() {
	a := uiapp.New()
	mygo.App.SetName(i18n.T("window_title"))

	mygo.App.WhenReady(func() {
		mainWin := mygo.NewWindow(mygo.WindowOptions{
			Title:     i18n.T("window_title"),
			Width:     720,
			Height:    560,
			MinWidth:  480,
			MinHeight: 400,
			StateKey:  "main",
			Content:   ui.View(a.View),
		})
		a.Win = mainWin
	})

	if err := mygo.App.Run(); err != nil {
		log.Fatal(err)
	}
}
