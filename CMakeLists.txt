cmake_minimum_required(VERSION 3.28)

project(kourier)

find_package(ECM 6.0 REQUIRED NO_MODULE)
set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH})
include(KDEInstallDirs)
include(ECMUninstallTarget)

include(ECMFindQmlModule)
ecm_find_qmlmodule(org.kde.kirigami REQUIRED)
find_package(KF6 REQUIRED COMPONENTS QQC2DesktopStyle)

add_custom_target(kourier
    ALL
    COMMAND cargo build --target-dir ${CMAKE_CURRENT_BINARY_DIR}
    BYPRODUCTS ${CMAKE_BINARY_DIR}/debug
)

install(
    PROGRAMS ${CMAKE_CURRENT_BINARY_DIR}/debug/kourier
    DESTINATION ${KDE_INSTALL_BINDIR}
)

install(
    FILES org.kourier.desktop
    DESTINATION ${KDE_INSTALL_APPDIR}
)

install(
    FILES src/svg/kourier.svg
    DESTINATION ${KDE_INSTALL_ICONDIR}/hicolor/scalable/apps
    RENAME kourier.svg
)
