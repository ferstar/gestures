package uiapp

import (
	"fmt"

	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/model"
)

// card is a rounded, bordered panel that groups related controls.
func card(c *ui.Context, t *ui.Theme, fn func()) {
	ui.Column(c).Gap(12).Padding(16).Radius(10).
		Background(t.Surface).Border(1, t.Border).Children(fn)
}

// statusPill shows a colored label, e.g. the service state in the header.
func statusPill(c *ui.Context, t *ui.Theme, label string, color ui.Color) {
	ui.Row(c).Padding(4, 12).Radius(999).Background(color).Children(func() {
		ui.Text(c, label).FontSize(12).Bold().TextColor(t.Background)
	})
}

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
			if a.Status.Active {
				statusPill(c, t, "Running", t.Success)
			} else {
				statusPill(c, t, "Stopped", t.TextMuted)
			}
			if ui.Button(c, "Refresh").Clicked() {
				a.RefreshStatus()
				a.ReloadConfig()
			}
		})

		ui.Row(c).Padding(8, 16).Background(t.Background).Children(func() {
			ui.Tabs(c, &a.Tab, "Service", "Gestures").Grow(1)
		})

		// Scrollable main content takes remaining height.
		ui.Scroll(c).Grow(1).Padding(16, 20).Gap(16).Children(func() {
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

	card(c, t, func() {
		ui.Form(c, func() {
			state := st.State
			if state == "" {
				state = "unknown"
			}
			ui.Field(c, "Status", func() {
				ui.Text(c, state).Bold().TextColor(stateColor(t, state))
			})
			ui.Field(c, "Display server", func() {
				ui.Text(c, st.DisplayServer)
			})
			ui.Field(c, "input group", func() {
				if !st.InputGroupOK {
					ui.Text(c, "could not check").TextColor(t.TextMuted)
				} else if st.InInputGroup {
					ui.Text(c, "yes — user is in input").TextColor(t.Success)
				} else {
					ui.Text(c, "no — add with: usermod -aG input $USER (then re-login)").TextColor(t.Danger)
				}
			})
			ui.Field(c, "Enabled", func() {
				if st.Enabled {
					ui.Text(c, "yes")
				} else {
					ui.Text(c, "no").TextColor(t.TextMuted)
				}
			})
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

// stateColor colors a systemd unit state: green when active, red when failed.
func stateColor(t *ui.Theme, state string) ui.Color {
	switch state {
	case "active":
		return t.Success
	case "failed":
		return t.Danger
	default:
		return t.TextMuted
	}
}

func (a *App) gesturesTab(c *ui.Context) {
	t := c.Theme()
	n := 0
	if a.Config != nil {
		n = len(a.Config.Gestures)
	}

	ui.Row(c).Gap(10).AlignItems(ui.Center).Children(func() {
		ui.Column(c).Grow(1).Gap(2).Children(func() {
			ui.Text(c, "Gestures").FontSize(16).Bold()
			ui.Textf(c, "%d configured", n).TextColor(t.TextMuted).FontSize(12)
		})
		if ui.PrimaryButton(c, "Add gesture").Clicked() {
			a.OpenEditor(-1)
		}
	})

	if n == 0 {
		card(c, t, func() {
			ui.Text(c, "No gestures yet.").Bold()
			ui.Text(c, "Click Add gesture, or run `gestures generate-config`.").TextColor(t.TextMuted).FontSize(12)
		})
		return
	}

	ui.Column(c).Gap(8).Children(func() {
		for i := 0; i < n; i++ {
			a.gestureRow(c, i)
		}
	})
}

func (a *App) gestureRow(c *ui.Context, i int) {
	t := c.Theme()
	g := a.Config.Gestures[i]

	title := g.ActionSummary()
	if g.Name != "" {
		title = g.Name
	}
	meta := g.TypeLabel() + " · " + fmt.Sprintf("%d fingers", g.Fingers)
	if g.Direction != "" {
		meta += " · " + g.Direction
	}

	ui.Row(c).Gap(12).Padding(12, 14).AlignItems(ui.Center).
		Background(t.Surface).Border(1, t.Border).Radius(10).Children(func() {
		ui.Column(c).Grow(1).Gap(4).Children(func() {
			ui.Text(c, title).FontSize(14).Bold()
			ui.Text(c, meta).FontSize(12).TextColor(t.TextMuted)
		})
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
					// mode is derived from a.EditMode; since mygo 0.3 bound input is
					// applied after the build, so commit the edit in OnChange.
					mode := string(a.EditMode)
					ui.Field(c, "Action type", func() {
						ui.Select(c, &mode, []string{string(model.ModeDrag), string(model.ModeExec)}).
							OnChange(func() { a.EditMode = model.ActionMode(mode) })
					})

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
