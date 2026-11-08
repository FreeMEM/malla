#!/bin/sh
# Instala Malla para el usuario actual: programa, icono y lanzador.
set -e
cd "$(dirname "$0")"
cargo install --path . --root "$HOME/.local" --force
ICONS="$HOME/.local/share/icons/hicolor"
install -Dm644 data/es.freemem.Malla.svg "$ICONS/scalable/apps/es.freemem.Malla.svg"
for s in 32 48 64 128 256; do
    mkdir -p "$ICONS/${s}x${s}/apps"
    rsvg-convert -w $s -h $s data/es.freemem.Malla.svg -o "$ICONS/${s}x${s}/apps/es.freemem.Malla.png"
done
mkdir -p "$HOME/.local/share/applications"
sed "s|^Exec=malla|Exec=$HOME/.local/bin/malla|" data/es.freemem.Malla.desktop \
    > "$HOME/.local/share/applications/es.freemem.Malla.desktop"
gtk-update-icon-cache -f -t "$ICONS" >/dev/null 2>&1 || true
update-desktop-database "$HOME/.local/share/applications" >/dev/null 2>&1 || true
echo "Malla instalada en $HOME/.local/bin/malla"
