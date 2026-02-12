gmessages_qt

Native KDE desktop client for Google Messages (SMS/RCS) built with Rust, CXX-Qt, and QML.

Auth data storage
- Auth data is stored by libgmessages-rs via `AuthDataStore::default_store()` in the user data directory.
- Path: `$(dirs::data_dir())/GMMessages/auth_data.json`
