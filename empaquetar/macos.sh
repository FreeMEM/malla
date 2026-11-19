#!/bin/sh
# Crea Malla.app (macOS) con el binario de target/release. Resultado en dist/Malla.app.
# macOS exige un Info.plist con la descripción de uso de Bluetooth; sin él cierra la app
# al conectar con el nodo. Antes: cargo build --release
set -e
cd "$(dirname "$0")/.."
APP=dist/Malla.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp target/release/malla "$APP/Contents/MacOS/malla"
cp data/macos/Info.plist "$APP/Contents/Info.plist"
# Firma ad-hoc: basta para ejecutarlo en tu propio Mac.
codesign --force --deep -s - "$APP"
echo "Listo: $APP"
