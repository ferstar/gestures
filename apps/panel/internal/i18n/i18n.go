package i18n

import (
	"os"
	"path/filepath"
	"strings"
	"sync"
)

type Lang string

const (
	LangEN Lang = "en"
	LangZH Lang = "zh"
)

var (
	mu          sync.RWMutex
	currentLang = initLang()
)

func resolveConfigFile() string {
	if path := os.Getenv("GESTURES_PANEL_LANG_FILE"); path != "" {
		return path
	}
	if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
		return filepath.Join(xdg, "gestures", "panel_lang")
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, ".config", "gestures", "panel_lang")
}

func initLang() Lang {
	cfg := resolveConfigFile()
	if cfg != "" {
		if data, err := os.ReadFile(cfg); err == nil {
			switch strings.TrimSpace(string(data)) {
			case string(LangZH):
				return LangZH
			case string(LangEN):
				return LangEN
			}
		}
	}
	return detectDefaultLang()
}

func detectDefaultLang() Lang {
	for _, env := range []string{"LC_ALL", "LC_MESSAGES", "LANG"} {
		val := os.Getenv(env)
		if strings.HasPrefix(strings.ToLower(val), "zh") {
			return LangZH
		}
	}
	return LangEN
}

func persist(l Lang) {
	cfg := resolveConfigFile()
	if cfg == "" {
		return
	}
	_ = os.MkdirAll(filepath.Dir(cfg), 0755)
	_ = os.WriteFile(cfg, []byte(string(l)+"\n"), 0644)
}

// Current returns current language.
func Current() Lang {
	mu.RLock()
	defer mu.RUnlock()
	return currentLang
}

// Set switches language and persists the choice.
func Set(l Lang) {
	mu.Lock()
	if l == LangZH {
		currentLang = LangZH
	} else {
		currentLang = LangEN
	}
	toPersist := currentLang
	mu.Unlock()

	persist(toPersist)
}

// Toggle toggles between en and zh and persists the choice.
func Toggle() {
	mu.Lock()
	if currentLang == LangZH {
		currentLang = LangEN
	} else {
		currentLang = LangZH
	}
	toPersist := currentLang
	mu.Unlock()

	persist(toPersist)
}

// T translates key with fallback to English or key itself.
func T(key string) string {
	mu.RLock()
	lang := currentLang
	mu.RUnlock()

	if dict, ok := messages[lang]; ok {
		if msg, ok := dict[key]; ok {
			return msg
		}
	}
	if dict, ok := messages[LangEN]; ok {
		if msg, ok := dict[key]; ok {
			return msg
		}
	}
	return key
}

var messages = map[Lang]map[string]string{
	LangEN: {
		"window_title":        "Gestures Panel",
		"tab_gestures":        "Gestures",
		"tab_settings":        "Settings",
		"running":             "Running",
		"stopped":             "Stopped",
		"start":               "Start",
		"stop":                "Stop",
		"new_gesture":         "+ New",
		"ready":               "Ready",
		"touchpad_accel":      "Touchpad & Acceleration",
		"accel_profile":       "Accel profile",
		"accel_speed":         "Accel speed",
		"override":            "Override",
		"device_default":      "Device default",
		"slower":              "Slower",
		"faster":              "Faster",
		"apply":               "Apply",
		"profile_default":     "Default",
		"profile_adaptive":    "Adaptive",
		"profile_flat":        "Flat",
		"service_system":      "Daemon & Environment",
		"status":              "Daemon status",
		"config_file":         "Config file",
		"display_server":      "Display server",
		"input_group":         "input group",
		"enabled":             "Desktop Autostart",
		"reload_config":       "Reload config",
		"reread_file":         "Re-read file",
		"refresh_status":      "Refresh status",
		"yes_in_input":        "yes — user is in input",
		"no_not_in_input":     "no — add with: usermod -aG input $USER (then re-login)",
		"could_not_check":     "could not check",
		"yes":                 "enabled (~/.config/autostart)",
		"no":                  "disabled",
		"tip_service":         "Gestures starts automatically via XDG desktop autostart on graphical login.",
		"no_gestures":         "No gestures yet.",
		"no_gestures_hint":    "Click '+ New' to add your first gesture, or run `gestures generate-config`.",
		"edit":                "Edit",
		"delete":              "Delete",
		"delete_title":        "Delete gesture?",
		"delete_confirm":      "Are you sure you want to delete \"%s\"? This cannot be undone.",
		"cancel":              "Cancel",
		"save":                "Save",
		"add_gesture":         "Add gesture",
		"edit_gesture":        "Edit gesture",
		"name":                "Name",
		"optional_label":      "optional label",
		"kind":                "Kind",
		"direction":           "Direction",
		"fingers":             "Fingers",
		"action_type":         "Action type",
		"drag":                "Drag",
		"command":             "Command",
		"mouseup_delay":       "Mouse-up delay (ms)",
		"drag_hint":           "Drag speed follows Touchpad settings",
		"extra_args":          "Extra args",
		"extra_args_hint":     "optional, appended",
		"action":              "Action",
		"command_hint":        "Stored as end=\"…\" — join program and args with spaces",
		"save_hint":           "Writes KDL to %s then reloads service.",
		"fingers_header":      "%d FINGERS",
		"swipe_up":            "Swipe up",
		"swipe_down":          "Swipe down",
		"swipe_right":         "Swipe right",
		"swipe_left":          "Swipe left",
		"swipe_up_right":      "Swipe up-right",
		"swipe_up_left":       "Swipe up-left",
		"swipe_down_right":    "Swipe down-right",
		"swipe_down_left":     "Swipe down-left",
		"swipe_any":           "Swipe any direction",
		"pinch_in":            "Pinch in",
		"pinch_out":           "Pinch out",
		"rotate_clockwise":    "Rotate clockwise",
		"rotate_counter":      "Rotate counterclockwise",
		"pinch_any":           "Pinch any direction",
		"hold":                "Hold",
		"dir_any":             "Any direction (✥)",
		"dir_n":               "Up (↑)",
		"dir_s":               "Down (↓)",
		"dir_e":               "Right (→)",
		"dir_w":               "Left (←)",
		"dir_ne":              "Up-right (↗)",
		"dir_nw":              "Up-left (↖)",
		"dir_se":              "Down-right (↘)",
		"dir_sw":              "Down-left (↙)",
		"dir_in":              "Pinch in (⇲)",
		"dir_out":             "Pinch out (⇱)",
		"dir_clockwise":        "Clockwise (↻)",
		"dir_counterclockwise": "Counterclockwise (↺)",
		"lang_switch":         "中",
	},
	LangZH: {
		"window_title":        "手势设置",
		"tab_gestures":        "手势列表",
		"tab_settings":        "设置与服务",
		"running":             "运行中",
		"stopped":             "已停止",
		"start":               "启动",
		"stop":                "停止",
		"new_gesture":         "+ 新建",
		"ready":               "就绪",
		"touchpad_accel":      "触控板与指针加速",
		"accel_profile":       "加速方案",
		"accel_speed":         "加速倍率",
		"override":            "自定义",
		"device_default":      "设备默认",
		"slower":              "更慢",
		"faster":              "更快",
		"apply":               "应用",
		"profile_default":     "默认",
		"profile_adaptive":    "自适应",
		"profile_flat":        "平直 (Flat)",
		"service_system":      "后台进程与环境",
		"status":              "进程状态",
		"config_file":         "配置文件",
		"display_server":      "显示服务器",
		"input_group":         "input 用户组",
		"enabled":             "登录桌面自启",
		"reload_config":       "重载配置",
		"reread_file":         "重新读取",
		"refresh_status":      "刷新状态",
		"yes_in_input":        "已就绪 (用户在 input 组)",
		"no_not_in_input":     "未加入 — 运行: usermod -aG input $USER (后重新登录)",
		"could_not_check":     "无法检测",
		"yes":                 "已开启 (~/.config/autostart)",
		"no":                  "未开启",
		"tip_service":         "提示: 守护进程随桌面登录通过 XDG Autostart 自动启动，无需常驻后台服务单元。",
		"no_gestures":         "暂无手势配置",
		"no_gestures_hint":    "点击右上角 '+ 新建' 添加手势，或执行 `gestures generate-config` 生成默认配置。",
		"edit":                "编辑",
		"delete":              "删除",
		"delete_title":        "删除手势确认",
		"delete_confirm":      "确定要删除手势 \"%s\" 吗？此操作无法撤销。",
		"cancel":              "取消",
		"save":                "保存",
		"add_gesture":         "新建手势",
		"edit_gesture":        "编辑手势",
		"name":                "名称",
		"optional_label":      "可选备注",
		"kind":                "手势类型",
		"direction":           "滑动方向",
		"fingers":             "手指数量",
		"action_type":         "触发动作",
		"drag":                "指针拖拽",
		"command":             "执行命令",
		"mouseup_delay":       "抬起延迟 (毫秒)",
		"drag_hint":           "拖拽速度受触控板加速设置影响",
		"extra_args":          "附加参数",
		"extra_args_hint":     "可选，追加到末尾",
		"action":              "动作命令",
		"command_hint":        "写入为 end=\"…\" — 命令与参数以空格分隔",
		"save_hint":           "将 KDL 写入至 %s 并自动重载系统服务。",
		"fingers_header":      "%d 指手势",
		"swipe_up":            "向上轻扫",
		"swipe_down":          "向下轻扫",
		"swipe_right":         "向右轻扫",
		"swipe_left":          "向左轻扫",
		"swipe_up_right":      "右上轻扫",
		"swipe_up_left":       "左上轻扫",
		"swipe_down_right":    "右下轻扫",
		"swipe_down_left":     "左下轻扫",
		"swipe_any":           "任意方向轻扫",
		"pinch_in":            "向内捏合 (缩紧)",
		"pinch_out":           "向外捏合 (张开)",
		"rotate_clockwise":    "顺时针旋转",
		"rotate_counter":      "逆时针旋转",
		"pinch_any":           "任意方向捏合",
		"hold":                "长按",
		"dir_any":             "任意方向 (✥)",
		"dir_n":               "向上 (↑)",
		"dir_s":               "向下 (↓)",
		"dir_e":               "向右 (→)",
		"dir_w":               "向左 (←)",
		"dir_ne":              "右上 (↗)",
		"dir_nw":              "左上 (↖)",
		"dir_se":              "右下 (↘)",
		"dir_sw":              "左下 (↙)",
		"dir_in":              "向内捏合 (⇲)",
		"dir_out":             "向外捏合 (⇱)",
		"dir_clockwise":        "顺时针旋转 (↻)",
		"dir_counterclockwise": "逆时针旋转 (↺)",
		"lang_switch":         "EN",
	},
}
