package uiapp

import (
	"fmt"
	"sort"
	"strings"

	"github.com/egoist/mygo/ui"

	"gestures-panel/internal/i18n"
	"gestures-panel/internal/model"
)

// card is a rounded, bordered panel that groups related controls.
func card(c *ui.Context, t *ui.Theme, fn func()) {
	ui.Column(c).Gap(12).Padding(16).Radius(10).
		Background(t.Surface).Border(1, t.Border).Children(fn)
}

// statusPill shows a colored label, e.g. the service state in the header.
func statusPill(c *ui.Context, t *ui.Theme, label string, color ui.Color) {
	ui.Row(c).Padding(3, 8).Radius(999).Background(t.Surface).Border(1, color).Children(func() {
		ui.Text(c, label).FontSize(11).Bold().TextColor(color)
	})
}

// View is the window content.
func (a *App) View(c *ui.Context) {
	t := c.Theme()
	ui.Column(c).Fill().Gap(0).Children(func() {
		// Header: compact, unified navigation and actions
		ui.Row(c).Padding(8, 16).Gap(12).AlignItems(ui.Center).Background(t.Surface).BorderWidth(0, 0, 1, 0).BorderColor(t.Border).Children(func() {
			// Main Tabs directly on the left
			ui.Tabs(c, &a.Tab, i18n.T("tab_gestures"), i18n.T("tab_settings"))

			ui.Spacer(c)

			// Service status & control group on the right
			ui.Row(c).Gap(6).AlignItems(ui.Center).Children(func() {
				if a.Status.Active {
					statusPill(c, t, i18n.T("running"), t.Success)
					if ui.Button(c, i18n.T("stop")).FontSize(12).Padding(3, 8).Clicked() {
						a.StopService()
					}
				} else {
					statusPill(c, t, i18n.T("stopped"), t.TextMuted)
					if ui.PrimaryButton(c, i18n.T("start")).FontSize(12).Padding(3, 8).Clicked() {
						a.StartService()
					}
				}
			})

			// Language Toggle
			if ui.Button(c, i18n.T("lang_switch")).FontSize(11).Padding(3, 8).Clicked() {
				i18n.Toggle()
				if a.Win != nil {
					a.Win.SetTitle(i18n.T("window_title"))
				}
			}

			// Add gesture quick action (shown on Gestures tab)
			if a.Tab == 0 {
				if ui.PrimaryButton(c, i18n.T("new_gesture")).FontSize(12).Padding(3, 10).Clicked() {
					a.OpenEditor(-1)
				}
			}
		})

		// Scrollable main content
		ui.Scroll(c).Grow(1).Padding(16, 20).Gap(16).Children(func() {
			switch a.Tab {
			case 1:
				a.settingsTab(c)
			default:
				a.gesturesTab(c)
			}
		})
	})

	if a.Editing {
		a.editModal(c)
	}
	if a.ConfirmDelete {
		name := ""
		if a.Config != nil && a.DeleteIndex >= 0 && a.DeleteIndex < len(a.Config.Gestures) {
			name = gestureTitle(a.Config.Gestures[a.DeleteIndex])
		}
		a.deleteConfirmModal(c, name)
	}
}

// deleteConfirmModal renders a clear destructive confirmation dialog.
func (a *App) deleteConfirmModal(c *ui.Context, name string) {
	t := c.Theme()
	ui.Modal(c, &a.ConfirmDelete, func() {
		ui.Column(c).Width(400).Gap(14).Padding(20).Children(func() {
			ui.Text(c, i18n.T("delete_title")).FontSize(16).Bold().TextColor(t.Danger)
			ui.Text(c, fmt.Sprintf(i18n.T("delete_confirm"), name)).FontSize(13).TextColor(t.Text)

			ui.Row(c).Gap(10).Justify(ui.End).Children(func() {
				if ui.Button(c, i18n.T("cancel")).Clicked() {
					a.ConfirmDelete = false
				}
				if ui.PrimaryButton(c, i18n.T("delete")).Background(t.Danger).TextColor(ui.Hex("#ffffff")).Clicked() {
					a.ConfirmDelete = false
					a.DeleteAt(a.DeleteIndex)
				}
			})
		})
	})
}

// settingsTab unifies Touchpad and Service system configurations.
func (a *App) settingsTab(c *ui.Context) {
	t := c.Theme()
	st := a.Status

	// Touchpad settings card
	ui.Column(c).Gap(8).Children(func() {
		ui.Text(c, i18n.T("touchpad_accel")).FontSize(14).Bold()
		card(c, t, func() {
			ui.Column(c).Gap(12).Children(func() {
				ui.Row(c).Gap(12).AlignItems(ui.Center).Children(func() {
					ui.Text(c, i18n.T("accel_profile")).FontSize(12).Bold().Width(110)
					segmentedChoice(c, &a.TouchProfile, model.AccelProfiles, []string{
						i18n.T("profile_default"),
						i18n.T("profile_adaptive"),
						i18n.T("profile_flat"),
					})
				})

				ui.Row(c).Gap(12).AlignItems(ui.Center).Children(func() {
					ui.Text(c, i18n.T("accel_speed")).FontSize(12).Bold().Width(110)
					ui.Switch(c, &a.TouchSpeedSet).Label(i18n.T("override"))
					ui.Text(c, i18n.T("override")).FontSize(12)
					ui.Spacer(c)
					value := i18n.T("device_default")
					if a.TouchSpeedSet {
						value = fmt.Sprintf("%.2f", a.TouchSpeed)
					}
					ui.Text(c, value).FontSize(12).TextColor(t.TextMuted)
				})

				ui.Row(c).Gap(12).AlignItems(ui.Center).Children(func() {
					ui.Text(c, "").Width(110) // Align indentation
					ui.Text(c, i18n.T("slower")).FontSize(11).TextColor(t.TextMuted)
					ui.Slider(c, &a.TouchSpeed, -1, 1).Grow(1).Disabled(!a.TouchSpeedSet)
					ui.Text(c, i18n.T("faster")).FontSize(11).TextColor(t.TextMuted)
				})

				ui.Row(c).Justify(ui.End).Children(func() {
					if ui.PrimaryButton(c, i18n.T("apply")).Clicked() {
						a.ApplyTouchpad()
					}
				})
			})
		})
	})

	// Service & System card
	ui.Column(c).Gap(8).Children(func() {
		ui.Text(c, i18n.T("service_system")).FontSize(14).Bold()
		card(c, t, func() {
			ui.Form(c, func() {
				state := st.State
				if state == "" {
					state = "unknown"
				}
				ui.Field(c, i18n.T("status"), func() {
					ui.Text(c, state).Bold().TextColor(stateColor(t, state))
				})
				ui.Field(c, i18n.T("config_file"), func() {
					ui.Text(c, a.ConfigPath).FontSize(12).TextColor(t.TextMuted)
				})
				ui.Field(c, i18n.T("display_server"), func() {
					ui.Text(c, st.DisplayServer)
				})
				ui.Field(c, i18n.T("input_group"), func() {
					if !st.InputGroupOK {
						ui.Text(c, i18n.T("could_not_check")).TextColor(t.TextMuted)
					} else if st.InInputGroup {
						ui.Text(c, i18n.T("yes_in_input")).TextColor(t.Success)
					} else {
						ui.Text(c, i18n.T("no_not_in_input")).TextColor(t.Danger)
					}
				})
				ui.Field(c, i18n.T("enabled"), func() {
					if st.Enabled {
						ui.Text(c, i18n.T("yes"))
					} else {
						ui.Text(c, i18n.T("no")).TextColor(t.TextMuted)
					}
				})
			})

			ui.Row(c).Gap(10).AlignItems(ui.Center).Children(func() {
				if ui.Button(c, i18n.T("reload_config")).Clicked() {
					a.ReloadService()
				}
				if ui.Button(c, i18n.T("reread_file")).Clicked() {
					a.ReloadConfig()
				}
				if ui.Button(c, i18n.T("refresh_status")).Clicked() {
					a.RefreshStatus()
				}
			})

			if st.Message != "" {
				ui.Text(c, st.Message).TextColor(t.TextMuted).FontSize(11)
			}
			ui.Text(c, i18n.T("tip_service")).
				TextColor(t.TextMuted).FontSize(11)
		})
	})
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

// selectMapping provides a dropdown choosing an internal value while showing localized labels.
func selectMapping(c *ui.Context, value *string, values, labels []string) {
	t := c.Theme()
	sel := ui.SelectBase(c, value)
	currentLabel := *value
	for i, v := range values {
		if v == *value && i < len(labels) {
			currentLabel = labels[i]
			break
		}
	}

	sel.Trigger.Padding(6, 12).Radius(6).Background(t.Surface).Border(1, t.Border).Children(func() {
		ui.Row(c).AlignItems(ui.Center).Children(func() {
			ui.Text(c, currentLabel).FontSize(13).Grow(1)
			ui.Text(c, "▾").FontSize(11).TextColor(t.TextMuted)
		})
	})
	sel.Popup(func(panel ui.Element) {
		panel.Padding(4).Radius(6).Background(t.Surface).Border(1, t.Border)
		for i, v := range values {
			label := v
			if i < len(labels) {
				label = labels[i]
			}
			item := sel.Item(v).Padding(6, 10).Radius(4)
			if item.Highlighted() {
				item.Background(t.Accent).TextColor(t.Background)
			}
			item.Children(func() { ui.Text(c, label).FontSize(13) })
		}
	})
}

// segmentedChoice maps presentation labels onto the existing string state.
func segmentedChoice(c *ui.Context, value *string, values, labels []string) {
	selected := 0
	for i, v := range values {
		if v == *value {
			selected = i
			break
		}
	}
	ui.Segmented(c, &selected, labels...).OnChange(func() { *value = values[selected] })
}

func (a *App) gesturesTab(c *ui.Context) {
	t := c.Theme()
	n := 0
	if a.Config != nil {
		n = len(a.Config.Gestures)
	}

	if n == 0 {
		ui.Column(c).Padding(48, 20).Gap(8).AlignItems(ui.Center).Children(func() {
			ui.Text(c, i18n.T("no_gestures")).FontSize(16).Bold()
			ui.Text(c, i18n.T("no_gestures_hint")).TextColor(t.TextMuted).FontSize(12)
		})
		return
	}

	// Sort only row indices: editing and persistence retain the original order.
	indices := make([]int, n)
	for i := range indices {
		indices[i] = i
	}
	sort.SliceStable(indices, func(i, j int) bool {
		return a.Config.Gestures[indices[i]].Fingers < a.Config.Gestures[indices[j]].Fingers
	})

	ui.Column(c).Gap(6).Children(func() {
		previous := -1
		for _, i := range indices {
			fingers := a.Config.Gestures[i].Fingers
			if fingers != previous {
				ui.Textf(c, i18n.T("fingers_header"), fingers).FontSize(11).Bold().TextColor(t.TextMuted).Padding(6, 0)
				previous = fingers
			}
			a.gestureRow(c, i)
		}
	})
}

func directionLabel(g model.Gesture) string {
	if g.Kind == model.KindHold {
		return "●"
	}
	arrows := map[string]string{
		"n": "↑", "s": "↓", "e": "→", "w": "←",
		"ne": "↗", "nw": "↖", "se": "↘", "sw": "↙", "any": "✥",
		"in": "⇲", "out": "⇱", "clockwise": "↻", "counterclockwise": "↺",
	}
	if arrow, ok := arrows[g.Direction]; ok {
		return arrow
	}
	return g.Direction
}

// gestureTitle supplies a list label without assigning a name to the config.
func gestureTitle(g model.Gesture) string {
	if strings.TrimSpace(g.Name) != "" {
		return g.Name
	}
	switch g.Kind {
	case model.KindHold:
		return i18n.T("hold")
	case model.KindPinch:
		switch g.Direction {
		case "in":
			return i18n.T("pinch_in")
		case "out":
			return i18n.T("pinch_out")
		case "clockwise":
			return i18n.T("rotate_clockwise")
		case "counterclockwise":
			return i18n.T("rotate_counter")
		default:
			return i18n.T("pinch_any")
		}
	case model.KindSwipe:
		if g.Mode() == model.ModeDrag {
			return i18n.T("drag")
		}
		directions := map[string]string{
			"n":  i18n.T("swipe_up"),
			"s":  i18n.T("swipe_down"),
			"e":  i18n.T("swipe_right"),
			"w":  i18n.T("swipe_left"),
			"ne": i18n.T("swipe_up_right"),
			"nw": i18n.T("swipe_up_left"),
			"se": i18n.T("swipe_down_right"),
			"sw": i18n.T("swipe_down_left"),
		}
		if direction, ok := directions[g.Direction]; ok {
			return direction
		}
		return i18n.T("swipe_any")
	default:
		return g.DisplayName()
	}
}

func (a *App) gestureRow(c *ui.Context, i int) {
	t := c.Theme()
	g := a.Config.Gestures[i]
	color := t.Accent
	switch g.TypeLabel() {
	case "exec":
		color = t.Success
	case "pinch":
		color = t.Warning
	case "hold":
		color = t.Danger
	}

	ui.Row(c).Gap(12).Padding(8, 12).AlignItems(ui.Center).
		Background(t.Surface).Border(1, t.Border).Radius(8).Children(func() {
		// Unified Icon Badge: combines direction icon and fingers into an intuitive visual
		ui.Row(c).Width(32).Height(32).Shrink(0).Justify(ui.Center).AlignItems(ui.Center).
			Radius(6).Background(t.Background).Border(1, color).Children(func() {
			ui.Text(c, directionLabel(g)).FontSize(15).Bold().TextColor(color)
		})

		// Gesture title & Action command (Click whole block to edit)
		if ui.ButtonBase(c).Grow(1).MinWidth(0).Label(i18n.T("edit") + " " + gestureTitle(g)).Children(func() {
			ui.Column(c).Gap(2).Grow(1).MinWidth(0).Children(func() {
				ui.Row(c).Gap(6).AlignItems(ui.Center).Children(func() {
					ui.Text(c, gestureTitle(g)).FontSize(13).Bold().SingleLine()
					ui.Text(c, g.TypeLabel()).FontSize(10).TextColor(color)
				})
				ui.Text(c, g.ActionSummary()).Font("monospace").FontSize(11).TextColor(t.TextMuted).SingleLine()
			})
		}).Clicked() {
			a.OpenEditor(i)
		}

		// Delete button only (edit is done by clicking the row)
		if ui.Button(c, i18n.T("delete")).FontSize(11).Padding(4, 6).
			Background(t.Surface).Border(0, t.Border).TextColor(t.Danger).Clicked() {
			a.DeleteIndex = i
			a.ConfirmDelete = true
		}
	})
}

func (a *App) editModal(c *ui.Context) {
	t := c.Theme()
	ui.Modal(c, &a.Editing, func() {
		ui.Column(c).Width(520).Gap(12).Padding(20).Children(func() {
			title := i18n.T("edit_gesture")
			if a.EditIndex < 0 {
				title = i18n.T("add_gesture")
			}
			ui.Text(c, title).FontSize(16).Bold()

			ui.Form(c, func() {
				ui.Field(c, i18n.T("name"), func() {
					ui.TextInput(c, &a.EditName).Placeholder(i18n.T("optional_label"))
				})
				ui.Field(c, i18n.T("kind"), func() {
					selected := 0
					kinds := []string{"swipe", "pinch", "hold"}
					for i, kind := range kinds {
						if kind == a.EditKindStr {
							selected = i
						}
					}
					ui.Segmented(c, &selected, kinds...).OnChange(func() {
						a.EditKindStr = kinds[selected]
						if a.EditKindStr == "pinch" {
							a.EditDir = "in"
						} else {
							a.EditDir = "any"
						}
					})
				})
				if a.EditKindStr != string(model.KindHold) {
					dirs := model.SwipeDirections
					if a.EditKindStr == string(model.KindPinch) {
						dirs = model.PinchDirections
					}
					labels := make([]string, len(dirs))
					for idx, d := range dirs {
						labels[idx] = i18n.T("dir_" + d)
					}
					ui.Field(c, i18n.T("direction"), func() {
						selectMapping(c, &a.EditDir, dirs, labels)
					})
				}
				ui.Field(c, i18n.T("fingers"), func() {
					ui.NumberInput(c, &a.EditFingers, 1, 10, 1)
				})

				if a.EditKindStr == string(model.KindSwipe) {
					mode := 0
					if a.EditMode == model.ModeExec {
						mode = 1
					}
					ui.Field(c, i18n.T("action_type"), func() {
						ui.Segmented(c, &mode, i18n.T("drag"), i18n.T("command")).
							OnChange(func() { a.EditMode = []model.ActionMode{model.ModeDrag, model.ModeExec}[mode] })
					})

					if a.EditMode == model.ModeDrag {
						ui.Field(c, i18n.T("mouseup_delay"), func() {
							ui.NumberInput(c, &a.EditDelay, 0, 5000, 50)
						}).Description(i18n.T("drag_hint"))
					} else {
						ui.Field(c, i18n.T("command"), func() {
							ui.TextInput(c, &a.EditCommand).Placeholder(`xdotool key super+Page_Up`)
						}).Description(i18n.T("command_hint"))
						ui.Field(c, i18n.T("extra_args"), func() {
							ui.TextInput(c, &a.EditArgsText).Placeholder(i18n.T("extra_args_hint"))
						})
					}
				} else if a.EditKindStr == string(model.KindPinch) {
					ui.Field(c, i18n.T("command"), func() {
						ui.TextInput(c, &a.EditCommand).Placeholder(`xdotool key ctrl+plus`)
					})
				} else {
					ui.Field(c, i18n.T("action"), func() {
						ui.TextInput(c, &a.EditCommand).Placeholder(`rofi -show drun`)
					})
				}
			})

			if a.EditError != "" {
				ui.Text(c, a.EditError).TextColor(t.Danger).FontSize(12)
			}

			ui.Row(c).Gap(10).Justify(ui.End).Children(func() {
				if ui.Button(c, i18n.T("cancel")).Clicked() {
					a.Editing = false
				}
				if ui.PrimaryButton(c, i18n.T("save")).Clicked() {
					a.CommitEdit()
				}
			})

			ui.Text(c, fmt.Sprintf(i18n.T("save_hint"), a.ConfigPath)).
				FontSize(11).TextColor(t.TextMuted)
		})
	})
}
