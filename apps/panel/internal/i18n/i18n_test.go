package i18n_test

import (
	"os"
	"path/filepath"
	"testing"

	"gestures-panel/internal/i18n"
)

func TestI18nToggleAndFallback(t *testing.T) {
	tmpDir := t.TempDir()
	langFile := filepath.Join(tmpDir, "panel_lang")
	t.Setenv("GESTURES_PANEL_LANG_FILE", langFile)

	i18n.Set(i18n.LangEN)
	if i18n.T("start") != "Start" {
		t.Fatalf("expected Start, got %s", i18n.T("start"))
	}
	i18n.Toggle()
	if i18n.Current() != i18n.LangZH {
		t.Fatalf("expected LangZH, got %s", i18n.Current())
	}
	if i18n.T("start") != "启动" {
		t.Fatalf("expected 启动, got %s", i18n.T("start"))
	}
	i18n.Set(i18n.LangEN)
}

func TestI18nPersistence(t *testing.T) {
	tmpDir := t.TempDir()
	langFile := filepath.Join(tmpDir, "panel_lang")
	t.Setenv("GESTURES_PANEL_LANG_FILE", langFile)

	i18n.Set(i18n.LangZH)
	data, err := os.ReadFile(langFile)
	if err != nil {
		t.Fatalf("failed to read persisted lang file: %v", err)
	}
	if string(data) != "zh\n" {
		t.Fatalf("expected zh\\n, got %q", string(data))
	}

	i18n.Toggle()
	data, err = os.ReadFile(langFile)
	if err != nil {
		t.Fatalf("failed to read persisted lang file: %v", err)
	}
	if string(data) != "en\n" {
		t.Fatalf("expected en\\n, got %q", string(data))
	}
	i18n.Set(i18n.LangEN)
}
