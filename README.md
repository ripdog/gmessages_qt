# gmessages_qt

A native KDE desktop client for Google Messages (SMS/RCS), built with Rust, CXX-Qt, and KDE Kirigami. It interfaces with the Google Messages web client APIs via `libgmessages-rs` to provide a fast and seamless native messaging experience on Linux.

## Features

- **Native UI**: Built with Qt6 and Kirigami for seamless integration with KDE Plasma and modern desktop environments.
- **Real-time Sync**: Features real-time background long-polling updates for incoming messages and conversation state.
- **Media Viewer**: Full-screen viewing of image attachments (supports "fit to window" and "actual-size" modes) with local caching.
- **Message Management**: Support for deleting messages and viewing detailed read receipts or status indicators (sending, sent, received, read).
- **System Integration**: Background daemon support (`--background` flag), providing system-tray persistence and native desktop notifications for incoming texts.

## Requirements

### Build Dependencies
- Rust (Cargo)
- CMake
- KDE Extra CMake Modules (`extra-cmake-modules`)
- Qt6 (`qt6-base`, `qt6-declarative`)
- KDE Kirigami (`kirigami`)

### Optional Runtime Dependencies
- FFmpeg (`ffmpeg`): Required for generating and displaying video thumbnails.

## Building & Installation

### Arch Linux (Recommended)
This repository includes a `PKGBUILD` configured to build and package the application correctly without any of the C++ and Rust `cxx-qt` linker errors caused by default `makepkg` LTO flag injections. To build and install the native Arch package in-place:

```bash
makepkg -si
```
This will automatically compile the release build, set up the `.desktop` launcher, and tie the installation to your package manager.

### Manual CMake Installation
If you are on another distribution, you can install the application using standard CMake commands:
```bash
cmake -B build -S . -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=Release
cmake --build build
sudo cmake --install build
```

### Development (Cargo)
You can directly run the application using Cargo during development:
```bash
cargo run
```

## Storage & Authentication

Auth data is stored securely by `libgmessages-rs` via `AuthDataStore::default_store()` in your local user data directory. This handles and maintains the pairing credentials to your phone.

- **Storage Path**: `~/.local/share/GMMessages/auth_data.json` (or your OS equivalent of `dirs::data_dir()`).
