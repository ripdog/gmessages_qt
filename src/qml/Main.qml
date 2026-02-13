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

    readonly property AppState appState: AppState
    {
    }
    readonly property SessionController sessionController: SessionController
    {
    }
    readonly property ConversationList conversationList: ConversationList
    {
    }
    readonly property MessageList messageListModel: MessageList
    {
    }
    property int selectedConversationIndex: -1
    property string selectedConversationName: ""
    property string selectedMeParticipantId: ""
    property string outgoingText: ""
    property int statusVisibleIndex: -1
    property int lastMessageCount: 0
    property real messageListRightGutter: Kirigami.Units.gridUnit * 2.5
    property var conversationActions: []
    property string pendingConversationFilter: ""

    Component.onCompleted: {
        appState.initialize()
    }
    globalDrawer: Kirigami.GlobalDrawer
    {
        id: conversationListDrawer
        title: "Conversations"
        titleIcon: "view-conversation-balloon"
        showHeaderWhenCollapsed: true
        modal: false
        actions: conversationActions
        header: Controls.ToolBar
        {
            contentItem: RowLayout {
                Layout.fillWidth: true
                Controls.ToolButton {
                    icon.name: "application-menu"
                    visible: appState.logged_in
                    checked: !conversationListDrawer.collapsed
                    onClicked: conversationListDrawer.collapsed = !conversationListDrawer.collapsed
                }
                Kirigami.SearchField {
                    visible: !conversationListDrawer.collapsed
                    Layout.fillWidth: true
                    onTextChanged: {
                        root.pendingConversationFilter = text
                        filterDebounce.restart()
                    }
                }
            }
        }
    }

    Timer {
        id: filterDebounce

        interval: 150
        repeat: false
        onTriggered: conversationList.apply_filter(root.pendingConversationFilter)
    }

    Instantiator {
        id: conversationActionFactory

        model: appState.logged_in ? conversationList : null
        onModelChanged: conversationActions = []
        // onObjectAdded: function (index, object) {
        //     if (!object) {
        //         return
        //     }
        //     conversationActions.push(object)
        //     if (!conversationListDrawer.actions || conversationListDrawer.actions.length !== conversationActions.length) {
        //         conversationListDrawer.actions = conversationActions
        //     }
        // }
        // onObjectRemoved: function (index, object) {
        //     if (!object) {
        //         return
        //     }
        //     const pos = conversationActions.indexOf(object)
        //     if (pos >= 0) {
        //         conversationActions.splice(pos, 1)
        //     }
        //     conversationListDrawer.actions = conversationActions
        // }
        delegate: Kirigami.Action
        {
            text: name
            enabled: !conversationList.loading
            icon.source: avatar_url
            icon.name: avatar_url.length > 0 ? "" : (is_group_chat ? "group" : "user-identity")
            onTriggered: {
                const convoId = conversationList.conversation_id(index)
                selectedConversationIndex = index
                selectedConversationName = name
                selectedMeParticipantId = conversationList.me_participant_id(index)
                statusVisibleIndex = -1
                lastMessageCount = 0
                messageListModel.load(convoId)
            }
        }
    }

    pageStack.initialPage: Kirigami.Page
    {
        title: selectedConversationName.length > 0 ? selectedConversationName : "Messages"

        Item {
            id: loggedOutView

            anchors.fill: parent
            visible: !appState.logged_in

            ColumnLayout {
                anchors.centerIn: parent
                spacing: Kirigami.Units.largeSpacing
                width: parent.width - Kirigami.Units.gridUnit * 2

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
        }


        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.largeSpacing

                Item {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: !messageListModel.loading && selectedConversationIndex >= 0

                ListView {
                    id: messageList

                    anchors.fill: parent
                    anchors.rightMargin: root.messageListRightGutter
                    model: messageListModel
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    rightMargin: root.messageListRightGutter
                    bottomMargin: Kirigami.Units.gridUnit
                    contentWidth: width - root.messageListRightGutter
                    verticalLayoutDirection: ListView.TopToBottom
                    spacing: Kirigami.Units.mediumSpacing

                    onCountChanged: {
                        if (count > 0) {
                            positionViewAtEnd()
                        }
                        if (!messageListModel.loading && count > lastMessageCount) {
                            statusVisibleIndex = count - 1
                        }
                        lastMessageCount = count
                    }

                    Controls.ScrollBar.vertical: Controls.ScrollBar
                    {
                        policy: Controls.ScrollBar.AsNeeded
                    }

                    delegate: Item {
                        id: messageDelegate

                        required property int index
                        required property string body
                        required property bool from_me
                        required property string time
                        required property string status

                        width: messageList.width - root.messageListRightGutter
                        height: messageRow.implicitHeight + metaContainer.height + Kirigami.Units.smallSpacing

                        TapHandler {
                            acceptedButtons: Qt.LeftButton
                            grabPermissions: PointerHandler.CanTakeOverFromAnything
                            onTapped: root.statusVisibleIndex = messageDelegate.index
                        }

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
                                    messageDelegate.width - avatar.width - Kirigami.Units.gridUnit * 4
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

                                    TapHandler {
                                        acceptedButtons: Qt.LeftButton
                                        onTapped: root.statusVisibleIndex = messageDelegate.index
                                    }
                                }
                            }
                        }

                        Item {
                            id: metaContainer

                            anchors.top: messageRow.bottom
                            anchors.right: messageDelegate.from_me ? parent.right : undefined
                            anchors.left: messageDelegate.from_me ? undefined : parent.left
                            anchors.topMargin: Kirigami.Units.smallSpacing
                            height: messageDelegate.index === root.statusVisibleIndex ? metaRow.implicitHeight : 0
                            width: metaRow.implicitWidth
                            clip: true

                            Row {
                                id: metaRow

                                spacing: Kirigami.Units.smallSpacing

                                Controls.Label {
                                    text: time
                                    color: Kirigami.Theme.disabledTextColor
                                    font.pixelSize: Kirigami.Units.gridUnit * 0.8
                                }
                                Controls.Label {
                                    text: "\u00B7"
                                    color: Kirigami.Theme.disabledTextColor
                                    font.pixelSize: Kirigami.Units.gridUnit * 0.8
                                    visible: messageDelegate.from_me
                                }
                                Image {
                                    width: 22
                                    height: 12
                                    fillMode: Image.PreserveAspectFit
                                    visible: messageDelegate.from_me
                                    source: messageDelegate.status === "read"
                                        ? "qrc:/svg/readIcon.svg"
                                        : messageDelegate.status === "received"
                                            ? "qrc:/svg/receivedIcon.svg"
                                            : messageDelegate.status === "sending"
                                                ? "qrc:/svg/sendingIcon.svg"
                                                : "qrc:/svg/sentIcon.svg"
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

                enabled: selectedConversationIndex >= 0 && !messageListModel.loading

                Controls.TextField {
                    placeholderText: "Type a message"
                    Layout.fillWidth: true
                    text: outgoingText
                    onTextChanged: outgoingText = text
                    onAccepted: {
                        if (outgoingText.trim().length > 0) {
                            messageListModel.send_message(outgoingText)
                            outgoingText = ""
                        }
                    }
                }
                Controls.Button {
                    text: "Send"
                    enabled: outgoingText.trim().length > 0
                    onClicked: {
                        messageListModel.send_message(outgoingText)
                        outgoingText = ""
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
                } else {
                    if (!sessionController.running) {
                        sessionController.start()
                    }
                    conversationList.load()
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
            if (appState.logged_in) {
                conversationList.load()
            }
        }
    }

    Connections {
        target: sessionController

        function onMessage_received(conversationId, participantId, body, transportType, messageId, tmpId, timestampMicros, statusCode) {
            messageListModel.handle_message_event(conversationId, participantId, body, transportType, messageId, tmpId, timestampMicros, statusCode)
        }
    }

    Connections {
        target: conversationList

        function onAuth_error(message) {
            appState.logout(message)
        }
    }

    Connections {
        target: messageListModel

        function onAuth_error(message) {
            appState.logout(message)
        }
    }
}
