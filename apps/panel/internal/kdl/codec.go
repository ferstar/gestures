// Package kdl implements a pragmatic reader/writer for ferstar/gestures KDL.
//
// On-disk shape (from ferstar/gestures config.rs + knuffel):
//
//	swipe direction="any" fingers=3 mouse-up-delay=500 acceleration=20
//	swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"
//	pinch direction="in" fingers=2 end="xdotool key ctrl+minus"
//	hold fingers=4 action="rofi -show drun"
//
// This is intentionally a subset codec: enough to round-trip real config files.
package kdl

import (
	"fmt"
	"os"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"

	"gestures-panel/internal/model"
)

// DefaultPath returns the preferred config path (~/.config/gestures.kdl).
func DefaultPath() string {
	if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
		return xdg + "/gestures.kdl"
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return "gestures.kdl"
	}
	return home + "/.config/gestures.kdl"
}

// CandidatePaths are search paths matching the Rust tool.
func CandidatePaths() []string {
	home := ""
	if h, err := os.UserHomeDir(); err == nil {
		home = h
	}
	xdg := os.Getenv("XDG_CONFIG_HOME")
	if xdg == "" && home != "" {
		xdg = home + "/.config"
	}
	var out []string
	if xdg != "" {
		out = append(out, xdg+"/gestures.kdl", xdg+"/gestures/gestures.kdl")
	}
	return out
}

// ResolvePath returns the first existing candidate, or DefaultPath if none exist.
func ResolvePath() string {
	for _, p := range CandidatePaths() {
		if st, err := os.Stat(p); err == nil && !st.IsDir() {
			return p
		}
	}
	return DefaultPath()
}

// Parse parses gestures.kdl content into a Config.
func Parse(src string) (*model.Config, error) {
	cfg := &model.Config{}
	lines := strings.Split(src, "\n")
	var pendingName string
	var pendingComments []string
	i := 0
	for i < len(lines) {
		raw := lines[i]
		trim := strings.TrimSpace(raw)
		if trim == "" {
			i++
			continue
		}
		if strings.HasPrefix(trim, "//") {
			comment := strings.TrimSpace(strings.TrimPrefix(trim, "//"))
			// Optional: "// name: foo" or bare comment used as gesture name.
			if strings.HasPrefix(strings.ToLower(comment), "name:") {
				pendingName = strings.TrimSpace(comment[len("name:"):])
			} else if pendingName == "" && comment != "" && !strings.HasPrefix(comment, "==") &&
				!strings.HasPrefix(comment, "Gestures") && !strings.HasPrefix(comment, "See ") &&
				!strings.HasPrefix(comment, "- ") && !strings.HasPrefix(comment, "Works ") &&
				!strings.HasPrefix(comment, "Uncomment") && !strings.HasPrefix(comment, "Hyprland") &&
				!strings.HasPrefix(comment, "i3/") && !strings.HasPrefix(comment, "GNOME") &&
				!strings.HasPrefix(comment, "Browser") && !strings.HasPrefix(comment, "Application") &&
				!strings.HasPrefix(comment, "Screenshot") && !strings.HasPrefix(comment, "3-Finger") &&
				!strings.HasPrefix(comment, "4-Finger") && !strings.HasPrefix(comment, "Pinch") &&
				!strings.HasPrefix(comment, "Hold") {
				// Keep as possible name only if short and not a section header.
				if len(comment) < 60 && !strings.Contains(comment, "====") {
					pendingName = comment
				} else {
					if len(cfg.Gestures) == 0 {
						cfg.HeaderComments = append(cfg.HeaderComments, comment)
					}
					pendingComments = append(pendingComments, comment)
				}
			} else {
				if len(cfg.Gestures) == 0 {
					cfg.HeaderComments = append(cfg.HeaderComments, comment)
				}
			}
			i++
			continue
		}
		if strings.HasPrefix(trim, "/*") {
			// Skip block comments (rare in samples).
			for i < len(lines) && !strings.Contains(lines[i], "*/") {
				i++
			}
			i++
			continue
		}

		// Support line continuations with trailing \
		joined := trim
		for strings.HasSuffix(joined, "\\") {
			joined = strings.TrimSuffix(joined, "\\")
			i++
			if i >= len(lines) {
				break
			}
			joined += " " + strings.TrimSpace(lines[i])
		}

		g, err := parseNode(joined)
		if err != nil {
			return nil, fmt.Errorf("line %d: %w", i+1, err)
		}
		if g != nil {
			g.Name = pendingName
			cfg.Gestures = append(cfg.Gestures, *g)
			pendingName = ""
			pendingComments = nil
		}
		i++
	}
	_ = pendingComments
	return cfg, nil
}

func parseNode(line string) (*model.Gesture, error) {
	line = strings.TrimSpace(line)
	if line == "" {
		return nil, nil
	}
	// Strip trailing inline // comment carefully (not inside quotes).
	line = stripInlineComment(line)

	r := &reader{s: line}
	r.skipSpace()
	name := r.readIdent()
	if name == "" {
		return nil, fmt.Errorf("expected node name")
	}
	kind := model.Kind(name)
	switch kind {
	case model.KindSwipe, model.KindPinch, model.KindHold:
	default:
		return nil, fmt.Errorf("unknown gesture node %q (want swipe/pinch/hold)", name)
	}
	g := &model.Gesture{Kind: kind}
	for {
		r.skipSpace()
		if r.eof() {
			break
		}
		key := r.readIdent()
		if key == "" {
			return nil, fmt.Errorf("expected property name near %q", r.rest())
		}
		r.skipSpace()
		if !r.consume('=') {
			return nil, fmt.Errorf("expected = after %s", key)
		}
		r.skipSpace()
		val, err := r.readValue()
		if err != nil {
			return nil, err
		}
		if err := applyProp(g, key, val); err != nil {
			return nil, err
		}
	}
	if g.Fingers == 0 {
		return nil, fmt.Errorf("%s: fingers is required", name)
	}
	if kind != model.KindHold && g.Direction == "" {
		return nil, fmt.Errorf("%s: direction is required", name)
	}
	return g, nil
}

func applyProp(g *model.Gesture, key, val string) error {
	switch key {
	case "direction":
		g.Direction = strings.ToLower(val)
	case "fingers":
		n, err := strconv.Atoi(val)
		if err != nil {
			return fmt.Errorf("fingers: %w", err)
		}
		g.Fingers = n
	case "acceleration":
		n, err := strconv.Atoi(val)
		if err != nil {
			return fmt.Errorf("acceleration: %w", err)
		}
		g.Acceleration = &n
	case "mouse-up-delay", "mouse_up_delay":
		n, err := strconv.ParseInt(val, 10, 64)
		if err != nil {
			return fmt.Errorf("mouse-up-delay: %w", err)
		}
		g.MouseUpDelay = &n
	case "start":
		g.Start = val
	case "update":
		g.Update = val
	case "end":
		g.End = val
	case "action":
		g.Action = val
	default:
		return fmt.Errorf("unknown property %q", key)
	}
	return nil
}

// Format writes a Config as gestures.kdl.
func Format(cfg *model.Config) string {
	var b strings.Builder
	if len(cfg.HeaderComments) > 0 {
		for _, c := range cfg.HeaderComments {
			b.WriteString("// ")
			b.WriteString(c)
			b.WriteByte('\n')
		}
		b.WriteByte('\n')
	} else {
		b.WriteString("// Gestures Configuration\n")
		b.WriteString("// Managed by gestures-panel — see https://github.com/ferstar/gestures\n\n")
	}
	for i, g := range cfg.Gestures {
		if i > 0 {
			b.WriteByte('\n')
		}
		if strings.TrimSpace(g.Name) != "" {
			b.WriteString("// name: ")
			b.WriteString(strings.TrimSpace(g.Name))
			b.WriteByte('\n')
		}
		b.WriteString(formatNode(g))
		b.WriteByte('\n')
	}
	return b.String()
}

func formatNode(g model.Gesture) string {
	var parts []string
	parts = append(parts, string(g.Kind))
	if g.Kind != model.KindHold {
		parts = append(parts, fmt.Sprintf("direction=%s", quoteIfNeeded(g.Direction)))
	}
	parts = append(parts, fmt.Sprintf("fingers=%d", g.Fingers))
	if g.Kind == model.KindSwipe {
		if g.MouseUpDelay != nil {
			parts = append(parts, fmt.Sprintf("mouse-up-delay=%d", *g.MouseUpDelay))
		}
		if g.Acceleration != nil {
			parts = append(parts, fmt.Sprintf("acceleration=%d", *g.Acceleration))
		}
	}
	if g.Start != "" {
		parts = append(parts, fmt.Sprintf("start=%s", quoteString(g.Start)))
	}
	if g.Update != "" {
		parts = append(parts, fmt.Sprintf("update=%s", quoteString(g.Update)))
	}
	if g.End != "" {
		parts = append(parts, fmt.Sprintf("end=%s", quoteString(g.End)))
	}
	if g.Action != "" {
		parts = append(parts, fmt.Sprintf("action=%s", quoteString(g.Action)))
	}
	return strings.Join(parts, " ")
}

func quoteString(s string) string {
	escaped := strings.ReplaceAll(s, `\`, `\\`)
	escaped = strings.ReplaceAll(escaped, `"`, `\"`)
	return `"` + escaped + `"`
}

func quoteIfNeeded(s string) string {
	for _, r := range s {
		if !unicode.IsLetter(r) && !unicode.IsDigit(r) && r != '_' && r != '-' {
			return quoteString(s)
		}
	}
	if s == "" {
		return `""`
	}
	return quoteString(s) // directions are always quoted in samples
}

// Load reads and parses a config file.
func Load(path string) (*model.Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return Parse(string(data))
}

// Save writes cfg to path, creating parent dirs as needed.
func Save(path string, cfg *model.Config) error {
	if err := os.MkdirAll(parentDir(path), 0o755); err != nil {
		return err
	}
	return os.WriteFile(path, []byte(Format(cfg)), 0o644)
}

func parentDir(path string) string {
	i := strings.LastIndexByte(path, '/')
	if i <= 0 {
		return "."
	}
	return path[:i]
}

// --- lexer helpers ---

type reader struct {
	s string
	i int
}

func (r *reader) eof() bool { return r.i >= len(r.s) }

func (r *reader) rest() string {
	if r.eof() {
		return ""
	}
	return r.s[r.i:]
}

func (r *reader) peek() (rune, int) {
	if r.eof() {
		return 0, 0
	}
	return utf8.DecodeRuneInString(r.s[r.i:])
}

func (r *reader) skipSpace() {
	for !r.eof() {
		ch, sz := r.peek()
		if ch != ' ' && ch != '\t' {
			return
		}
		r.i += sz
	}
}

func (r *reader) consume(want byte) bool {
	if r.eof() || r.s[r.i] != want {
		return false
	}
	r.i++
	return true
}

func (r *reader) readIdent() string {
	start := r.i
	for !r.eof() {
		ch, sz := r.peek()
		if unicode.IsLetter(ch) || unicode.IsDigit(ch) || ch == '_' || ch == '-' {
			r.i += sz
			continue
		}
		break
	}
	return r.s[start:r.i]
}

func (r *reader) readValue() (string, error) {
	if r.eof() {
		return "", fmt.Errorf("expected value")
	}
	if r.s[r.i] == '"' {
		return r.readQuoted()
	}
	// bare number or ident
	start := r.i
	for !r.eof() {
		ch, sz := r.peek()
		if unicode.IsSpace(ch) {
			break
		}
		r.i += sz
	}
	return r.s[start:r.i], nil
}

func (r *reader) readQuoted() (string, error) {
	if !r.consume('"') {
		return "", fmt.Errorf("expected quote")
	}
	var b strings.Builder
	for !r.eof() {
		ch, sz := r.peek()
		if ch == '"' {
			r.i += sz
			return b.String(), nil
		}
		if ch == '\\' {
			r.i += sz
			if r.eof() {
				return "", fmt.Errorf("unterminated escape")
			}
			esc, esz := r.peek()
			r.i += esz
			switch esc {
			case '"', '\\', '/':
				b.WriteRune(esc)
			case 'n':
				b.WriteByte('\n')
			case 't':
				b.WriteByte('\t')
			default:
				b.WriteRune(esc)
			}
			continue
		}
		r.i += sz
		b.WriteRune(ch)
	}
	return "", fmt.Errorf("unterminated string")
}

func stripInlineComment(line string) string {
	inQuote := false
	escape := false
	for i := 0; i < len(line); i++ {
		c := line[i]
		if escape {
			escape = false
			continue
		}
		if c == '\\' && inQuote {
			escape = true
			continue
		}
		if c == '"' {
			inQuote = !inQuote
			continue
		}
		if !inQuote && c == '/' && i+1 < len(line) && line[i+1] == '/' {
			return strings.TrimSpace(line[:i])
		}
	}
	return line
}
