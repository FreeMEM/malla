#!/bin/sh
# Sube PKGBUILD y .SRCINFO a AUR. Necesita cuenta en aur.archlinux.org
# con tu clave SSH pública añadida en "My Account".
set -e
cd "$(dirname "$0")"
makepkg --printsrcinfo > .SRCINFO
TMP=$(mktemp -d)
git clone -q ssh://aur@aur.archlinux.org/malla.git "$TMP"
cp PKGBUILD .SRCINFO "$TMP"/
cd "$TMP"
git add PKGBUILD .SRCINFO
VER=$(sed -n 's/^\tpkgver = //p' .SRCINFO)-$(sed -n 's/^\tpkgrel = //p' .SRCINFO)
git commit -q -m "malla $VER"
git push -q origin HEAD:master
echo "Publicado en https://aur.archlinux.org/packages/malla"
rm -rf "$TMP"
