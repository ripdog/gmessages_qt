# Maintainer: ripdog <ripdog@example.com>
pkgname=kourier
pkgver=0.1.1
pkgrel=1
pkgdesc="An unofficial native KDE client for Google Messages"
arch=('x86_64' 'aarch64')
license=('GPL-3.0-or-later')
depends=('qt6-base' 'qt6-declarative' 'kirigami')
optdepends=('ffmpeg: Video thumbnail generation')
makedepends=('cargo' 'cmake' 'extra-cmake-modules')
source=()
options=('!lto' '!buildflags')

build() {
  cd "$startdir"
  cmake -B build -S . \
    -DCMAKE_INSTALL_PREFIX=/usr \
    -DCMAKE_BUILD_TYPE=None \
    -DKDE_INSTALL_LIBDIR=lib
  cmake --build build
}

package() {
  cd "$startdir"
  DESTDIR="$pkgdir" cmake --install build
}
