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

    readonly property AppState appState: AppState {}
    readonly property SessionController sessionController: SessionController {}
    readonly property ConversationList conversationList: ConversationList {}
    readonly property MessageList messageListModel: MessageList {}
    property int selectedConversationIndex: -1
    property string selectedConversationName: ""
    property string selectedMeParticipantId: ""

    Component.onCompleted: {
        appState.initialize()
    }

    pageStack.initialPage: Kirigami.Page {
        title: "Messages"

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
                                onTextChanged: conversationList.apply_filter(text)
                            }

                            Controls.ScrollView {
                                id: contactsScroll

                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true



                                contentItem: ListView {
                                    id: contactsList

                                    model: conversationList
                                    clip: true
                                    boundsBehavior: Flickable.StopAtBounds
                                    maximumFlickVelocity: 9000
                                    flickDeceleration: 4000
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true

                                    delegate: Rectangle {
                                        width: contactsList.width
                                        height: Kirigami.Units.gridUnit * 3
                                        color: Qt.rgba(0, 0, 0, 0)

                                        Controls.AbstractButton {
                                            anchors.fill: parent
                                            onClicked: {
                                                const convoId = conversationList.conversation_id(index)
                                                selectedConversationIndex = index
                                                selectedConversationName = name
                                                selectedMeParticipantId = conversationList.me_participant_id(index)
                                                messageListModel.load(convoId)
                                            }
                                        }

                                        Rectangle {
                                            anchors.fill: parent
                                            color: Kirigami.Theme.highlightColor
                                            opacity: 0.12
                                            visible: index === selectedConversationIndex
                                        }

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.margins: Kirigami.Units.smallSpacing
                                            spacing: Kirigami.Units.smallSpacing

                                            RowLayout {
                                                Layout.fillWidth: true

                                                Controls.Label {
                                                    text: name
                                                    font.bold: true
                                                    Layout.fillWidth: true
                                                }
                                                Controls.Label {
                                                    text: time
                                                    color: Kirigami.Theme.disabledTextColor
                                                }
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
                                Controls.ScrollBar.vertical: Controls.ScrollBar {
                                    policy: Controls.ScrollBar.AsNeeded
                                    anchors.top: contactsList.top
                                    anchors.bottom: contactsList.bottom
                                    anchors.left: contactsList.right
                                }
                            }

                            Item {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                visible: conversationList.loading || contactsList.count === 0

                                ColumnLayout {
                                    anchors.centerIn: parent
                                    spacing: Kirigami.Units.largeSpacing

                                    Controls.BusyIndicator {
                                        running: conversationList.loading
                                        visible: conversationList.loading
                                        Layout.alignment: Qt.AlignHCenter
                                    }

                                    Controls.Label {
                                        text: "No conversations"
                                        color: Kirigami.Theme.disabledTextColor
                                        visible: !conversationList.loading && contactsList.count === 0
                                        Layout.alignment: Qt.AlignHCenter
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
                                    text: selectedConversationName.length > 0 ? selectedConversationName : "Conversation"
                                    font.bold: true
                                    Layout.fillWidth: true
                                }
                                
                            }
                        }

                        Item {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            visible: !messageListModel.loading && selectedConversationIndex >= 0

                            ListView {
                                id: messageList

                                anchors.fill: parent
                                anchors.rightMargin: Kirigami.Units.smallSpacing
                                model: messageListModel
                                clip: true
                                boundsBehavior: Flickable.StopAtBounds
                                verticalLayoutDirection: ListView.BottomToTop
                                spacing: Kirigami.Units.mediumSpacing

                                Controls.ScrollBar.vertical: Controls.ScrollBar {
                                    policy: Controls.ScrollBar.AsNeeded
                                }

                                delegate: Item {
                                    id: messageDelegate

                                    required property int index
                                    required property string body
                                    required property bool from_me

                                    width: messageList.width
                                    height: messageRow.implicitHeight

                                    Row {
                                        id: messageRow

                                        anchors.left: messageDelegate.from_me ? undefined : parent.left
                                        anchors.right: messageDelegate.from_me ? parent.right : undefined
                                        anchors.leftMargin: Kirigami.Units.smallSpacing
                                        anchors.rightMargin: Kirigami.Units.smallSpacing
                                        spacing: Kirigami.Units.smallSpacing
                                        layoutDirection: messageDelegate.from_me ? Qt.RightToLeft : Qt.LeftToRight

                                        Rectangle {
                                            id: avatar

                                            width: Kirigami.Units.gridUnit * 2
                                            height: Kirigami.Units.gridUnit * 2
                                            radius: width / 2
                                            color: messageDelegate.from_me ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                                            anchors.bottom: parent.bottom

                                            Controls.Label {
                                                anchors.centerIn: parent
                                                text: messageDelegate.from_me ? "Me" : "?"
                                                font.pixelSize: Kirigami.Units.gridUnit * 0.7
                                                color: "white"
                                            }
                                        }

                                        Rectangle {
                                            id: bubble

                                            width: Math.min(
                                                bubbleText.implicitWidth + Kirigami.Units.gridUnit * 2,
                                                messageList.width - avatar.width - Kirigami.Units.gridUnit * 4
                                            )
                                            height: bubbleText.implicitHeight + Kirigami.Units.gridUnit * 1.2
                                            radius: Kirigami.Units.gridUnit * 0.6
                                            color: messageDelegate.from_me ? Kirigami.Theme.highlightColor : Kirigami.Theme.alternateBackgroundColor
                                            border.width: messageDelegate.from_me ? 0 : 1
                                            border.color: messageDelegate.from_me ? "transparent" : Kirigami.Theme.disabledTextColor

                                            TextEdit {
                                                id: bubbleText

                                                anchors.fill: parent
                                                anchors.margins: Kirigami.Units.gridUnit * 0.6
                                                text: messageDelegate.body
                                                color: messageDelegate.from_me ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
                                                wrapMode: Text.WordWrap
                                                readOnly: true
                                                selectByMouse: true
                                                selectedTextColor: messageDelegate.from_me ? Kirigami.Theme.textColor : Kirigami.Theme.highlightedTextColor
                                                selectionColor: messageDelegate.from_me ? Kirigami.Theme.backgroundColor : Kirigami.Theme.highlightColor
                                                font.pointSize: Kirigami.Theme.defaultFont.pointSize
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        Item {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            visible: messageListModel.loading

                            Controls.BusyIndicator {
                                anchors.centerIn: parent
                                running: true
                            }
                        }

                        Item {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            visible: selectedConversationIndex < 0 && !messageListModel.loading

                            Controls.Label {
                                anchors.centerIn: parent
                                text: "Select a conversation to view messages"
                                color: Kirigami.Theme.disabledTextColor
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

    }

    Connections {
        target: appState

        function onLogged_inChanged() {
            if (appState.logged_in && loginDialog.visible) {
                loginDialog.close()
            }
            if (appState.logged_in && !sessionController.running) {
                sessionController.start()
                conversationList.load()
            }
        }
    }

    Connections {
        target: loginDialog

        function onOpened() {
            conversationList.load()
        }
    }
}
