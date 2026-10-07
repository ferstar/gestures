package uiapp

import (
	"fmt"

	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/model"
)

// View is the window content.
func (a *App) View(c *ui.Context) {
	t := c.Theme()
	// Outer column fills the window. Header/tabs/footer keep intrinsic
	// height; only the content area Grow(1)s. Do not put Fill() on those
	// siblings — Fill sets height to 100% and starves the content panel.
	ui.Column(c).Fill().Gap(0).Children(func() {
		// Header
		ui.Row(c).Padding(16, 20).Gap(12).AlignItems(ui.Center).Background(t.Surface).Children(func() {
			ui.Column(c).Grow(1).Gap(2).Children(func() {
				ui.Text(c, "Gestures Panel").FontSize(18).Bold()
				ui.Text(c, a.ConfigPath).FontSize(12).TextColor(t.TextMuted)
			})
			running := a.Status.Active
			label := "Stopped"
			if running {
				label = "Running"
			}
			ui.Badge(c, label)
			if ui.Button(c, "Refresh").Clicked() {
				a.RefreshStatus()
				a.ReloadConfig()
			}
		})

		ui.Row(c).Padding(0, 16).Background(t.Background).Children(func() {
			ui.Tabs(c, &a.Tab, "Service", "Gestures").Grow(1)
		})

		// Scrollable main content takes remaining height.
		ui.Scroll(c).Grow(1).Padding(16, 20).Gap(12).Children(func() {
			switch a.Tab {
			case 0:
				a.serviceTab(c)
			default:
				a.gesturesTab(c)
			}
		})

		// Status bar
		ui.Row(c).Padding(10, 16).Gap(8).Background(t.Surface).Children(func() {
			if a.StatusErr != "" {
				ui.Text(c, a.StatusErr).TextColor(t.Danger).FontSize(12).Grow(1)
			} else if a.StatusMsg != "" {
				ui.Text(c, a.StatusMsg).TextColor(t.TextMuted).FontSize(12).Grow(1)
			} else {
				ui.Text(c, "Ready").TextColor(t.TextMuted).FontSize(12).Grow(1)
			}
		})
	})

	if a.Editing {
		a.editModal(c)
	}
	if a.ConfirmDelete {
		idx := ui.AlertDialog(c, &a.ConfirmDelete, "Delete gesture?",
			"Remove this gesture from the config and save?",
			"Cancel", "Delete")
		if idx == 1 {
			a.DeleteAt(a.DeleteIndex)
		}
	}
}

func (a *App) serviceTab(c *ui.Context) {
	t := c.Theme()
	st := a.Status

	ui.Text(c, "Service").FontSize(16).Bold()
	ui.Text(c, "Controls the gestures.service user unit (systemctl --user).").TextColor(t.TextMuted).FontSize(12)

	ui.Form(c, func() {
		state := st.State
		if state == "" {
			state = "unknown"
		}
		ui.Field(c, "Status", func() {
			ui.Text(c, state)
		})
		ui.Field(c, "Display server", func() {
			ui.Text(c, st.DisplayServer)
		})
		ui.Field(c, "input group", func() {
			if !st.InputGroupOK {
				ui.Text(c, "could not check")
			} else if st.InInputGroup {
				ui.Text(c, "yes — user is in input")
			} else {
				ui.Text(c, "no — add with: usermod -aG input $USER (then re-login)")
			}
		})
		ui.Field(c, "Enabled", func() {
			if st.Enabled {
				ui.Text(c, "yes")
			} else {
				ui.Text(c, "no")
			}
		})
	})

	ui.Row(c).Gap(10).Children(func() {
		if ui.PrimaryButton(c, "Start").Clicked() {
			a.StartService()
		}
		if ui.Button(c, "Stop").Clicked() {
			a.StopService()
		}
		if ui.Button(c, "Reload config").Clicked() {
			a.ReloadService()
		}
		if ui.Button(c, "Re-read file").Clicked() {
			a.ReloadConfig()
		}
	})

	if st.Message != "" {
		ui.Text(c, st.Message).TextColor(t.TextMuted).FontSize(12)
	}
	ui.Text(c, "Tip: install the unit with `gestures install-service` then enable with systemctl --user enable --now gestures.service.").
		TextColor(t.TextMuted).FontSize(12)
}

func (a *App) gesturesTab(c *ui.Context) {
	t := c.Theme()
	ui.Row(c).Gap(10).AlignItems(ui.Center).Children(func() {
		ui.Text(c, "Gestures").FontSize(16).Bold().Grow(1)
		if ui.PrimaryButton(c, "Add gesture").Clicked() {
			a.OpenEditor(-1)
		}
	})

	n := 0
	if a.Config != nil {
		n = len(a.Config.Gestures)
	}
	if n == 0 {
		ui.Text(c, "No gestures yet. Click Add gesture, or run `gestures generate-config`.").TextColor(t.TextMuted)
		return
	}

	ui.Column(c).Gap(8).Children(func() {
		// Header row
		ui.Row(c).Gap(8).Padding(4, 0).Children(func() {
			ui.Text(c, "Type").Bold().FontSize(11).TextColor(t.TextMuted).Width(56)
			ui.Text(c, "Fingers").Bold().FontSize(11).TextColor(t.TextMuted).Width(56)
			ui.Text(c, "Dir").Bold().FontSize(11).TextColor(t.TextMuted).Width(48)
			ui.Text(c, "Action").Bold().FontSize(11).TextColor(t.TextMuted).Grow(1)
			ui.Text(c, "").Width(120)
		})
		for i := 0; i < n; i++ {
			a.gestureRow(c, i)
		}
	})
}

func (a *App) gestureRow(c *ui.Context, i int) {
	t := c.Theme()
	g := a.Config.Gestures[i]
	ui.Row(c).Gap(8).Padding(8, 10).AlignItems(ui.Center).
		Background(t.Surface).Radius(8).Children(func() {
		ui.Text(c, g.TypeLabel()).Width(56).FontSize(13)
		ui.Textf(c, "%d", g.Fingers).Width(56).FontSize(13)
		dir := g.Direction
		if dir == "" {
			dir = "—"
		}
		ui.Text(c, dir).Width(48).FontSize(13)
		summary := g.ActionSummary()
		if g.Name != "" {
			summary = g.Name + " — " + summary
		}
		ui.Text(c, summary).Grow(1).FontSize(13).TextColor(t.TextMuted)
		ui.Row(c).Gap(6).Children(func() {
			if ui.Button(c, "Edit").Clicked() {
				a.OpenEditor(i)
			}
			if ui.Button(c, "Delete").Clicked() {
				a.DeleteIndex = i
				a.ConfirmDelete = true
			}
		})
	})
}

func (a *App) editModal(c *ui.Context) {
	t := c.Theme()
	ui.Modal(c, &a.Editing, func() {
		ui.Column(c).Width(440).Gap(12).Padding(20).Children(func() {
			title := "Edit gesture"
			if a.EditIndex < 0 {
				title = "Add gesture"
			}
			ui.Text(c, title).FontSize(18).Bold()

			ui.Form(c, func() {
				ui.Field(c, "Name", func() {
					ui.TextInput(c, &a.EditName).Placeholder("optional label")
				})
				ui.Field(c, "Kind", func() {
					ui.Select(c, &a.EditKindStr, []string{
						string(model.KindSwipe),
						string(model.KindPinch),
						string(model.KindHold),
					})
				})
				if a.EditKindStr != string(model.KindHold) {
					dirs := model.SwipeDirections
					if a.EditKindStr == string(model.KindPinch) {
						dirs = model.PinchDirections
					}
					ui.Field(c, "Direction", func() {
						ui.Select(c, &a.EditDir, dirs)
					})
				}
				ui.Field(c, "Fingers", func() {
					ui.NumberInput(c, &a.EditFingers, 1, 10, 1)
				})

				if a.EditKindStr == string(model.KindSwipe) {
					mode := string(a.EditMode)
					ui.Field(c, "Action type", func() {
						ui.Select(c, &mode, []string{string(model.ModeDrag), string(model.ModeExec)})
					})
					a.EditMode = model.ActionMode(mode)

					if a.EditMode == model.ModeDrag {
						ui.Field(c, "Acceleration", func() {
							ui.Slider(c, &a.EditAccel, 1, 40)
							ui.Textf(c, "%.0f", a.EditAccel).FontSize(12).TextColor(t.TextMuted)
						}).Description("Rust tool uses ~20 for 2× speed")
						ui.Field(c, "Mouse-up delay (ms)", func() {
							ui.NumberInput(c, &a.EditDelay, 0, 5000, 50)
						})
					} else {
						ui.Field(c, "Command", func() {
							ui.TextInput(c, &a.EditCommand).Placeholder(`xdotool key super+Page_Up`)
						}).Description(`Stored as end="…" — join program and args with spaces`)
						ui.Field(c, "Extra args", func() {
							ui.TextInput(c, &a.EditArgsText).Placeholder("optional, appended")
						})
					}
				} else if a.EditKindStr == string(model.KindPinch) {
					ui.Field(c, "Command", func() {
						ui.TextInput(c, &a.EditCommand).Placeholder(`xdotool key ctrl+plus`)
					})
				} else {
					ui.Field(c, "Action", func() {
						ui.TextInput(c, &a.EditCommand).Placeholder(`rofi -show drun`)
					})
				}
			})

			if a.EditError != "" {
				ui.Text(c, a.EditError).TextColor(t.Danger).FontSize(12)
			}

			ui.Row(c).Gap(10).Justify(ui.End).Children(func() {
				if ui.Button(c, "Cancel").Clicked() {
					a.Editing = false
				}
				if ui.PrimaryButton(c, "Save").Clicked() {
					a.CommitEdit()
				}
			})

			ui.Text(c, fmt.Sprintf("Writes KDL to %s then reloads the user service.", a.ConfigPath)).
				FontSize(11).TextColor(t.TextMuted)
		})
	})
}
