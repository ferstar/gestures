package i18n_test

import (
	"testing"

	"gestures-panel/internal/i18n"
)

func TestI18nToggleAndFallback(t *testing.T) {
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
