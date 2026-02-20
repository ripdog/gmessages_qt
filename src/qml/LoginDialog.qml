import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Controls.Dialog {
    id: loginDialog

    title: "Log in"
    modal: true
    standardButtons: Controls.Dialog.Close
    width: Math.min(root.width * 0.70, Kirigami.Units.gridUnit * 26)

    onOpened: {
        appState.start_login()
        sessionController.start()
    }

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Controls.Label {
            text: appState.status_message
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        Rectangle {
            color: Kirigami.Theme.alternateBackgroundColor
            radius: Kirigami.Units.smallSpacing
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: Kirigami.Units.gridUnit * 10
            Layout.preferredHeight: Kirigami.Units.gridUnit * 10

            Image {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.smallSpacing
                fillMode: Image.PreserveAspectFit
                source: appState.qr_svg_data_url
                visible: appState.qr_svg_data_url.length > 0
            }

            Controls.Label {
                anchors.centerIn: parent
                text: "Waiting for QR…"
                color: Kirigami.Theme.disabledTextColor
                visible: appState.qr_svg_data_url.length === 0
            }
        }
    }

    onClosed: {
        if (!appState.logged_in) {
            appState.cancel_login()
            appState.status_message = "Not logged in"
        } else {
            if (!sessionController.running) {
                sessionController.start()
            }
            conversationList.load()
        }
    }
}
