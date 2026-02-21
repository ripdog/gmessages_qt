Kourier

This is a Rust/CXX/Qt project using QML for the UI. It is a native KDE desktop client for Google Messages (the web UI for the Android app with SMS and RCS). The old name was gmessages-qt.

As this tech stack is rather obscure, there are various references for you to learn from here. 
In /references/kirigami-gallery, the various QML widgets which Kirigami are showcased. Whereever possible, use a Kirigami widget.

In /references/cxx-qt/examples, the cxx-qt examples live. Inspect them for details on how to interface qt and rust.

You can also retrieve the cxx-qt book. Particularly useful chapters might be https://kdab.github.io/cxx-qt/book/concepts/generated_qobject.html and https://kdab.github.io/cxx-qt/book/concepts/types.html

We already have a Rust library that handles the protocol communication: libgmessages-rs.

Standards
- Maintainable, readable, well-structured code
- Good engineering practices (clear ownership, separation of concerns, explicit interfaces)
- Fix bugs at the root cause, not by hacks or papering over symptoms
- High test coverage for critical logic and regressions
- Always check for errors, fix them, then build the app before ending a turn

Learnings
- For CXX-Qt: keep at least one `#[cxx_qt::bridge]` in `src/lib.rs` and include only `src/lib.rs` in `build.rs` `.files(...)`; referencing `src/main.rs` can break the build with `no #[cxx::bridge] module found`
- For QR refresh: regenerate the QR by creating a fresh `AuthData` and new pairing stream each cycle; reusing the same auth keeps the QR unchanged and the pairing stream times out

File Structure & Refactoring Rules
- NEVER create mega-files!
- Files (especially Rust and QML) should be kept strictly under 500-800 lines.
- When you are prompted to create a new feature, always consider abstracting the logical sections out to separate files or modules to ensure files stay small and easy to navigate.
- For Rust, modularize into `pub mod x` directories.
- For QML, extract `Kirigami.Page`, `Controls.Dialog`, or complex `Item`/`Component`s into their own `.qml` files instead of inline `Component { }` blocks. Ensure dependency injection parameters (like context properties) are configured to keep it decoupled.
