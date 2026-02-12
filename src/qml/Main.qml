import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import org.gmessages_qt

Kirigami.ApplicationWindow {
    id: root

    title: "gmessages"

    minimumWidth: Kirigami.Units.gridUnit * 40
    minimumHeight: Kirigami.Units.gridUnit * 24
    width: minimumWidth
    height: minimumHeight

    pageStack.initialPage: initPage

    Component {
        id: initPage

        Kirigami.Page {
            title: "Messages"

            readonly property AppState appState: AppState {}
            readonly property SessionController sessionController: SessionController {}

            Component.onCompleted: {
                appState.initialize()
            }

            RowLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.largeSpacing

                Rectangle {
                    id: contactsPane

                    color: Kirigami.Theme.backgroundColor
                    border.color: Kirigami.Theme.disabledTextColor
                    border.width: 1

                    Layout.preferredWidth: Math.max(root.width * 0.30, Kirigami.Units.gridUnit * 14)
                    Layout.fillHeight: true

                    Item {
                        id: contactsContent

                        anchors.fill: parent

                        ColumnLayout {
                            id: loggedOutView

                            anchors.centerIn: parent
                            spacing: Kirigami.Units.largeSpacing
                            width: parent.width - Kirigami.Units.gridUnit * 2
                            visible: !appState.logged_in

                            Controls.Label {
                                text: "Not logged in, Log in to get started"
                                wrapMode: Text.WordWrap
                                horizontalAlignment: Text.AlignHCenter
                                Layout.fillWidth: true
                            }
                            Controls.Button {
                                text: "Log in"
                                Layout.alignment: Qt.AlignHCenter
                                enabled: !appState.login_in_progress
                                onClicked: loginDialog.open()
                            }
                        }

                        ColumnLayout {
                            id: loggedInView

                            visible: appState.logged_in
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.largeSpacing
                            spacing: Kirigami.Units.largeSpacing

                            Controls.Label {
                                text: "Contacts"
                                font.bold: true
                                Layout.fillWidth: true
                            }

                            Controls.TextField {
                                placeholderText: "Search"
                                Layout.fillWidth: true
                            }

                            ListView {
                                id: contactsList

                                model: ListModel {
                                    ListElement { name: "Alice"; preview: "Typing..." }
                                    ListElement { name: "Brandon"; preview: "See you soon" }
                                    ListElement { name: "Casey"; preview: "Thanks!" }
                                    ListElement { name: "Drew"; preview: "On my way" }
                                    ListElement { name: "Evelyn"; preview: "Got it" }
                                }
                                clip: true
                                Layout.fillWidth: true
                                Layout.fillHeight: true

                                delegate: Rectangle {
                                    width: ListView.view.width
                                    height: Kirigami.Units.gridUnit * 3
                                    color: Qt.rgba(0, 0, 0, 0)

                                    ColumnLayout {
                                        anchors.fill: parent
                                        anchors.margins: Kirigami.Units.smallSpacing
                                        spacing: Kirigami.Units.smallSpacing

                                        Controls.Label {
                                            text: name
                                            font.bold: true
                                            Layout.fillWidth: true
                                        }
                                        Controls.Label {
                                            text: preview
                                            color: Kirigami.Theme.disabledTextColor
                                            elide: Text.ElideRight
                                            Layout.fillWidth: true
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    id: conversationPane

                    color: Kirigami.Theme.backgroundColor
                    border.color: Kirigami.Theme.disabledTextColor
                    border.width: 1

                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.largeSpacing
                        spacing: Kirigami.Units.largeSpacing

                        Rectangle {
                            color: Kirigami.Theme.alternateBackgroundColor
                            radius: Kirigami.Units.smallSpacing
                            Layout.fillWidth: true
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 3

                            RowLayout {
                                anchors.fill: parent
                                anchors.margins: Kirigami.Units.largeSpacing

                                Controls.Label {
                                    text: "Alice"
                                    font.bold: true
                                    Layout.fillWidth: true
                                }
                                Controls.Label {
                                    text: "Online"
                                    color: Kirigami.Theme.disabledTextColor
                                }
                            }
                        }

                        ListView {
                            id: messageList

                            model: ListModel {
                                ListElement { fromMe: false; body: "Hey! How's the project going?" }
                                ListElement { fromMe: true; body: "Good progress. UI is taking shape." }
                                ListElement { fromMe: false; body: "Nice. Want to review later?" }
                                ListElement { fromMe: true; body: "Sure, after I finish the placeholder screens." }
                            }
                            clip: true
                            spacing: Kirigami.Units.smallSpacing
                            Layout.fillWidth: true
                            Layout.fillHeight: true

                            delegate: RowLayout {
                                width: ListView.view.width
                                spacing: Kirigami.Units.largeSpacing

                                Item { Layout.fillWidth: model.fromMe }

                                Rectangle {
                                    color: model.fromMe ? Kirigami.Theme.highlightColor : Kirigami.Theme.alternateBackgroundColor
                                    radius: Kirigami.Units.smallSpacing
                                    Layout.preferredWidth: Math.min(ListView.view.width * 0.70, Kirigami.Units.gridUnit * 22)

                                    Controls.Label {
                                        text: body
                                        color: model.fromMe ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
                                        wrapMode: Text.WordWrap
                                        padding: Kirigami.Units.largeSpacing
                                    }
                                }

                                Item { Layout.fillWidth: !model.fromMe }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.largeSpacing

                            Controls.TextField {
                                placeholderText: "Type a message"
                                Layout.fillWidth: true
                            }
                            Controls.Button {
                                text: "Send"
                            }
                        }
                    }
                }
            }

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
                            text: "Waiting for QR"
                            color: Kirigami.Theme.disabledTextColor
                            visible: appState.qr_svg_data_url.length === 0
                        }
                    }

                }

                onClosed: {
                    if (!appState.logged_in) {
                        appState.status_message = "Not logged in"
                    }
                }
            }

            Connections {
                target: appState

                function onLogged_inChanged() {
                    if (appState.logged_in && loginDialog.visible) {
                        loginDialog.close()
                    }
                    if (appState.logged_in && !sessionController.running) {
                        sessionController.start()
                    }
                }
            }
        }
    }
}
