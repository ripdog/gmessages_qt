use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};
use cxx_qt_lib_extras::QApplication;
use gmessages_qt as _;
use std::env;

fn main() {
    cxx_qt::init_crate!(gmessages_qt);
    cxx_qt::init_qml_module!("org.gmessages_qt");

    let mut app = QApplication::new();

    // To associate the executable to the installed desktop file
    QGuiApplication::set_desktop_file_name(&QString::from("org.gmessages_qt"));

    // To ensure the style is set correctly
    if env::var("QT_QUICK_CONTROLS_STYLE").is_err() {
        QQuickStyle::set_style(&QString::from("org.kde.desktop"));
    }

    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/org/gmessages_qt/src/qml/Main.qml"));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
