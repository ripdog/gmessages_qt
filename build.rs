use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("org.gmessages_qt").qml_files(&["src/qml/Main.qml"]),
    )
    .files(["src/lib.rs"])
    .build();
}
