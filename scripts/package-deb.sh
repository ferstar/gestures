#!/usr/bin/env bash
set -euo pipefail

# Package gestures and gestures-panel into a unified Debian package (.deb)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    VERSION=$(grep '^version =' "${ROOT_DIR}/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')
fi

# Detect architecture
ARCH="$(dpkg --print-architecture 2>/dev/null || uname -m)"
case "$ARCH" in
    x86_64|amd64)
        DEB_ARCH="amd64"
        RUST_TARGET="x86_64-unknown-linux-gnu"
        ;;
    aarch64|arm64)
        DEB_ARCH="arm64"
        RUST_TARGET="aarch64-unknown-linux-gnu"
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

echo "==> Packaging Gestures unified deb: version=${VERSION}, arch=${DEB_ARCH}"

# Locate or build binaries
GESTURES_BIN="${ROOT_DIR}/target/${RUST_TARGET}/release/gestures"
if [ ! -f "$GESTURES_BIN" ]; then
    GESTURES_BIN="${ROOT_DIR}/target/release/gestures"
fi

PANEL_BIN="${ROOT_DIR}/apps/panel/build/gestures-panel"

if [ ! -f "$GESTURES_BIN" ]; then
    echo "==> Building gestures binary (cargo)..."
    cargo build --release --manifest-path "${ROOT_DIR}/Cargo.toml"
    GESTURES_BIN="${ROOT_DIR}/target/release/gestures"
fi

if [ ! -f "$PANEL_BIN" ]; then
    echo "==> Building gestures-panel binary (go)..."
    (cd "${ROOT_DIR}/apps/panel" && go build -o "${PANEL_BIN}" .)
fi

# Build directory structure
BUILD_DIR="$(mktemp -d -t gestures-deb-XXXXXX)"
trap 'rm -rf "${BUILD_DIR}"' EXIT

mkdir -p "${BUILD_DIR}/DEBIAN"
mkdir -p "${BUILD_DIR}/usr/bin"
mkdir -p "${BUILD_DIR}/etc/xdg/autostart"
mkdir -p "${BUILD_DIR}/usr/share/applications"
mkdir -p "${BUILD_DIR}/usr/share/icons/hicolor/512x512/apps"

# Copy binaries and strip
install -m 755 "$GESTURES_BIN" "${BUILD_DIR}/usr/bin/gestures"
install -m 755 "$PANEL_BIN" "${BUILD_DIR}/usr/bin/gestures-panel"
strip "${BUILD_DIR}/usr/bin/gestures" 2>/dev/null || true
strip "${BUILD_DIR}/usr/bin/gestures-panel" 2>/dev/null || true

# Copy desktop and icon assets
install -m 644 "${ROOT_DIR}/apps/panel/resources/icon.png" "${BUILD_DIR}/usr/share/icons/hicolor/512x512/apps/gestures-panel.png"

cat << 'EOF' > "${BUILD_DIR}/usr/share/applications/gestures-panel.desktop"
[Desktop Entry]
Type=Application
Name=Gestures
Comment=Touchpad gestures configuration panel
Exec=gestures-panel
Icon=gestures-panel
Categories=Settings;HardwareSettings;Utility;
Terminal=false
StartupNotify=true
EOF

# Install system-wide autostart desktop entry (runs gestures start in graphical session)
cat << 'EOF' > "${BUILD_DIR}/etc/xdg/autostart/gestures.desktop"
[Desktop Entry]
Type=Application
Name=Gestures Daemon
Comment=Touchpad gestures daemon
Exec=gestures start
Terminal=false
NoDisplay=true
X-GNOME-Autostart-Phase=Applications
X-GNOME-Autostart-enabled=true
EOF

# Generate control file
cat << EOF > "${BUILD_DIR}/DEBIAN/control"
Package: gestures
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${DEB_ARCH}
Maintainer: ferstar <https://github.com/ferstar/gestures>
Depends: libinput10, libxdo3, libgtk-3-0 | libgtk-3-0t64, libwebkit2gtk-4.1-0
Description: Touchpad gestures daemon and graphical settings panel
 Gestures brings smooth touchpad gesture controls and 3-finger drag to Linux.
 Automatically launches via desktop autostart on user graphical login.
EOF

OUT_DIR="${ROOT_DIR}/dist"
mkdir -p "${OUT_DIR}"
DEB_FILE="${OUT_DIR}/gestures_${VERSION}_${DEB_ARCH}.deb"

dpkg-deb --root-owner-group --build "${BUILD_DIR}" "${DEB_FILE}"
echo "==> Successfully created Debian package: ${DEB_FILE}"
ls -lh "${DEB_FILE}"
