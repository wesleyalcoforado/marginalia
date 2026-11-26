#!/bin/sh
# Monta dist/Marginalia.app a partir do build de release (macOS).
# Uso: scripts/empacotar_mac.sh [icone.png]   (padrao: o pacote crates/ui/assets/app/Marginalia.icon)
# A PDFium vai em Contents/Frameworks, onde o app a procura (docs/pdfium.md).
set -eu
cd "$(dirname "$0")/.."

[ -f vendor/pdfium/lib/libpdfium.dylib ] || python3 scripts/baixar_pdfium.py
cargo build --release -p ui

VERSAO=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/ui/Cargo.toml | head -1)
APP=dist/Marginalia.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"

cp target/release/marginalia "$APP/Contents/MacOS/marginalia"
cp vendor/pdfium/lib/libpdfium.dylib "$APP/Contents/Frameworks/"
cp vendor/pdfium/LICENSE "$APP/Contents/Frameworks/LICENSE-pdfium.txt"

# Icone: o macOS 26 so mascara bem em todos os tamanhos o icone compilado de um pacote .icon (Assets.car, via actool,
# que vem com o Xcode); um .icns solto sai quadrado nos tamanhos grandes. Sem actool, cai no .icns do iconutil.
ICONE_XML=""
ICONE_PKG=crates/ui/assets/app/Marginalia.icon
if [ -z "${1:-}" ] && xcrun --find actool >/dev/null 2>&1; then
  TMP=$(mktemp -d)
  xcrun actool "$ICONE_PKG" --compile "$APP/Contents/Resources" --output-format human-readable-text \
    --notices --warnings --errors --output-partial-info-plist "$TMP/icone.plist" --app-icon Marginalia \
    --include-all-app-icons --enable-on-demand-resources NO --development-region en --target-device mac \
    --minimum-deployment-target 11.0 --platform macosx >/dev/null
  ICONE_XML="<key>CFBundleIconFile</key><string>Marginalia</string><key>CFBundleIconName</key><string>Marginalia</string>"
else
  ICONE=${1:-crates/ui/assets/app/marginalia-macos.png}
  SET=$(mktemp -d)/icone.iconset
  mkdir -p "$SET"
  for t in 16 32 128 256 512; do
    sips -z $t $t "$ICONE" --out "$SET/icon_${t}x${t}.png" >/dev/null
    sips -z $((t * 2)) $((t * 2)) "$ICONE" --out "$SET/icon_${t}x${t}@2x.png" >/dev/null
  done
  iconutil -c icns "$SET" -o "$APP/Contents/Resources/Marginalia.icns"
  ICONE_XML="<key>CFBundleIconFile</key><string>Marginalia</string>"
fi

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Marginalia</string>
  <key>CFBundleDisplayName</key><string>Marginalia</string>
  <key>CFBundleIdentifier</key><string>app.marginalia</string>
  <key>CFBundleExecutable</key><string>marginalia</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSAO</string>
  <key>CFBundleVersion</key><string>$VERSAO</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  $ICONE_XML
</dict></plist>
PLIST

# assinatura ad-hoc (sem conta de desenvolvedor): basta para rodar nesta maquina
codesign --force --deep --sign - "$APP"
echo "Pronto: $APP"
