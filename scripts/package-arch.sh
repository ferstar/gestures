#!/usr/bin/env bash
set -euo pipefail

# Build an Arch Linux package (.pkg.tar.zst) locally from current repo sources

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    VERSION=$(grep '^version =' "${ROOT_DIR}/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')
fi

echo "==> Building Arch Linux package (.pkg.tar.zst) for version=${VERSION}"

TMP_DIR="$(mktemp -d -t gestures-arch-XXXXXX)"
trap 'rm -rf "${TMP_DIR}"' EXIT

OUT_DIR="${ROOT_DIR}/dist"
mkdir -p "${OUT_DIR}"

cat << EOF > "${TMP_DIR}/PKGBUILD"
# Maintainer: ferstar <zhangjianfei3@gmail.com>
pkgname=gestures
pkgver=${VERSION}
pkgrel=1
pkgdesc="Fast libinput-based touchpad gestures daemon and GUI configuration panel"
arch=('x86_64' 'aarch64')
url="https://github.com/ferstar/gestures"
license=('MIT')
depends=('libinput' 'xdotool' 'systemd-libs' 'gtk3' 'webkit2gtk-4.1')
options=('!strip')

package() {
    install -Dm755 "${ROOT_DIR}/target/release/gestures" "\$pkgdir/usr/bin/gestures"
    install -Dm755 "${ROOT_DIR}/apps/panel/build/gestures-panel" "\$pkgdir/usr/bin/gestures-panel"

    install -Dm644 /dev/stdin "\$pkgdir/etc/xdg/autostart/gestures.desktop" << 'AUTOSTART_EOF'
[Desktop Entry]
Type=Application
Name=Gestures Daemon
Comment=Touchpad gestures daemon
Exec=gestures start
Terminal=false
NoDisplay=true
X-GNOME-Autostart-Phase=Applications
X-GNOME-Autostart-enabled=true
AUTOSTART_EOF

    install -Dm644 /dev/stdin "\$pkgdir/usr/share/applications/gestures-panel.desktop" << 'DESKTOP_EOF'
[Desktop Entry]
Type=Application
Name=Gestures
Comment=Touchpad gestures configuration panel
Exec=gestures-panel
Icon=gestures-panel
Categories=Settings;HardwareSettings;Utility;
Terminal=false
StartupNotify=true
DESKTOP_EOF

    install -Dm644 "${ROOT_DIR}/apps/panel/resources/icon.png" "\$pkgdir/usr/share/icons/hicolor/512x512/apps/gestures-panel.png"
    install -Dm644 "${ROOT_DIR}/LICENSE" "\$pkgdir/usr/share/licenses/\$pkgname/LICENSE"
}
EOF

# Ensure local binaries exist
if [ ! -f "${ROOT_DIR}/target/release/gestures" ]; then
    echo "==> Building gestures binary (cargo)..."
    cargo build --release --manifest-path "${ROOT_DIR}/Cargo.toml"
fi

if [ ! -f "${ROOT_DIR}/apps/panel/build/gestures-panel" ]; then
    echo "==> Building gestures-panel binary (go)..."
    (cd "${ROOT_DIR}/apps/panel" && go build -o build/gestures-panel .)
fi

(cd "${TMP_DIR}" && makepkg -f --nodeps)

cp "${TMP_DIR}"/gestures-*.pkg.tar.zst "${OUT_DIR}/"
echo "==> Successfully created Arch package in ${OUT_DIR}:"
ls -lh "${OUT_DIR}"/gestures-*.pkg.tar.zst
