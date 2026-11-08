#!/bin/sh
# Crea el paquete de Arch (.pkg.tar.zst) en un contenedor Arch limpio,
# sin las optimizaciones para la CPU local. Resultado en dist/.
set -e
cd "$(dirname "$0")/.."
mkdir -p dist
if command -v podman >/dev/null; then ENGINE=podman; else ENGINE=docker; fi
$ENGINE run --rm -v "$PWD/empaquetar/aur":/aur:ro,Z -v "$PWD/dist":/dist:Z docker.io/library/archlinux:latest bash -euc '
    pacman -Syu --noconfirm --needed base-devel rust gtk4 libadwaita libshumate dbus >/dev/null
    useradd -m build
    cp /aur/PKGBUILD /home/build/
    chown -R build /home/build
    su build -c "cd /home/build && makepkg -f --noconfirm >/dev/null"
    cp /home/build/*.pkg.tar.zst /dist/
'
ls -l dist/*.pkg.tar.zst
