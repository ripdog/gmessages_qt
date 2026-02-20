use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("org.gmessages_qt").qml_files(&[
            "src/qml/Main.qml",
            "src/qml/MessageDelegate.qml",
            "src/qml/MediaViewerDialog.qml",
            "src/qml/LoginDialog.qml",
        ]),
    )
    .files(["src/lib.rs"])
    .qrc("src/qml/resources.qrc")
    .build();
}
