# Maintainer: ripdog <ripdog@example.com>
pkgname=gmessages-qt
pkgver=0.1.0
pkgrel=1
pkgdesc="A Kirigami-based Google Messages Client"
arch=('x86_64' 'aarch64')
license=('GPL-3.0-or-later')
depends=('qt6-base' 'qt6-declarative' 'kirigami')
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
