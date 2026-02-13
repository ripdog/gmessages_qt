import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.delegates as Delegates
import org.gmessages_qt

Kirigami.ApplicationWindow {
    id: root

    title: selectedConversationName.length > 0 ? selectedConversationName + " — gmessages" : "gmessages"

    minimumWidth: Kirigami.Units.gridUnit * 25
    minimumHeight: Kirigami.Units.gridUnit * 20
    width: Kirigami.Units.gridUnit * 55
    height: Kirigami.Units.gridUnit * 35

    wideScreen: width > Kirigami.Units.gridUnit * 45

    readonly property AppState appState: AppState {}
    readonly property SessionController sessionController: SessionController {}
    readonly property ConversationList conversationList: ConversationList {}
    readonly property MessageList messageListModel: MessageList {}

    property int selectedConversationIndex: -1
    property string selectedConversationName: ""
    property string selectedMeParticipantId: ""
    property string outgoingText: ""
    property int statusVisibleIndex: -1
    property int lastMessageCount: 0
    property string pendingConversationFilter: ""

    // ── pageStack configuration ──────────────────────────────────
    pageStack {
        initialPage: appState.logged_in ? conversationListComponent : welcomeComponent
        columnView.columnResizeMode: root.pageStack.wideMode
            ? Kirigami.ColumnView.DynamicColumns
            : Kirigami.ColumnView.SingleColumn
        globalToolBar {
            style: Kirigami.ApplicationHeaderStyle.ToolBar
            showNavigationButtons: root.pageStack.currentIndex > 0
                ? Kirigami.ApplicationHeaderStyle.ShowBackButton
                : 0
        }
    }

    Component.onCompleted: {
        appState.initialize()
    }

    // ── Welcome / logged-out page ────────────────────────────────
    Component {
        id: welcomeComponent

        Kirigami.Page {
            title: "gmessages"

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.gridUnit * 4

                icon.name: "message-new"
                text: "Welcome to gmessages"
                explanation: "Log in with your Google Messages account to get started."

                helpfulAction: Kirigami.Action {
                    icon.name: "system-log-in"
                    text: "Log in"
                    enabled: !appState.login_in_progress
                    onTriggered: loginDialog.open()
                }
            }
        }
    }

    // ── Conversation list page ───────────────────────────────────
    Component {
        id: conversationListComponent

        Kirigami.Page {
            id: conversationListPage
            title: "Conversations"
            padding: 0

            Kirigami.ColumnView.minimumWidth: Kirigami.Units.gridUnit * 16
            Kirigami.ColumnView.maximumWidth: Kirigami.Units.gridUnit * 26
            Kirigami.ColumnView.preferredWidth: Kirigami.Units.gridUnit * 22

            // Search bar in the header
            header: Controls.ToolBar {
                contentItem: RowLayout {
                    Kirigami.SearchField {
                        Layout.fillWidth: true
                        placeholderText: "Search conversations…"
                        onTextChanged: {
                            root.pendingConversationFilter = text
                            filterDebounce.restart()
                        }
                    }
                }
            }

            // Conversation list
            Controls.ScrollView {
                anchors.fill: parent

                ListView {
                    id: conversationListView

                    model: root.conversationList
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    topMargin: Math.round(Kirigami.Units.smallSpacing / 2)
                    reuseItems: true

                    delegate: Delegates.RoundedItemDelegate {
                        id: conversationDelegate

                        required property int index
                        required property string name
                        required property string preview
                        required property string avatar_url
                        required property bool is_group_chat
                        required property bool unread

                        width: conversationListView.width
                        highlighted: conversationDelegate.index === root.selectedConversationIndex

                        onClicked: {
                            const convoId = root.conversationList.conversation_id(conversationDelegate.index)
                            root.selectedConversationIndex = conversationDelegate.index
                            root.selectedConversationName = conversationDelegate.name
                            root.selectedMeParticipantId = root.conversationList.me_participant_id(conversationDelegate.index)
                            root.statusVisibleIndex = -1
                            root.lastMessageCount = 0
                            root.messageListModel.load(convoId)

                            // Push the conversation page if not already shown
                            if (root.pageStack.depth < 2) {
                                root.pageStack.push(conversationPageComponent)
                            }
                            root.pageStack.currentIndex = 1
                        }

                        contentItem: RowLayout {
                            spacing: Kirigami.Units.largeSpacing

                            // Avatar
                            Item {
                                Layout.preferredWidth: Kirigami.Units.gridUnit * 2.5
                                Layout.preferredHeight: Kirigami.Units.gridUnit * 2.5

                                Image {
                                    anchors.fill: parent
                                    source: conversationDelegate.avatar_url
                                    fillMode: Image.PreserveAspectCrop
                                    visible: conversationDelegate.avatar_url.length > 0

                                    layer.enabled: true
                                    layer.effect: Item {
                                        property var source
                                        ShaderEffect {
                                            anchors.fill: parent
                                            property var src: source
                                        }
                                    }
                                }
                                Kirigami.Icon {
                                    anchors.fill: parent
                                    source: conversationDelegate.is_group_chat ? "group" : "user-identity"
                                    visible: conversationDelegate.avatar_url.length === 0
                                }
                            }

                            // Name + preview
                            ColumnLayout {
                                spacing: 0
                                Layout.fillWidth: true

                                Controls.Label {
                                    text: conversationDelegate.name
                                    elide: Text.ElideRight
                                    font.weight: conversationDelegate.unread ? Font.Bold : Font.Normal
                                    textFormat: Text.PlainText
                                    Layout.fillWidth: true
                                }

                                Controls.Label {
                                    text: conversationDelegate.preview
                                    elide: Text.ElideRight
                                    font: Kirigami.Theme.smallFont
                                    color: Kirigami.Theme.disabledTextColor
                                    textFormat: Text.PlainText
                                    Layout.fillWidth: true
                                }
                            }

                            // Unread indicator dot
                            Rectangle {
                                Layout.preferredWidth: Kirigami.Units.smallSpacing * 2
                                Layout.preferredHeight: Kirigami.Units.smallSpacing * 2
                                Layout.alignment: Qt.AlignVCenter
                                radius: width / 2
                                color: Kirigami.Theme.highlightColor
                                visible: conversationDelegate.unread
                            }
                        }
                    }

                    Kirigami.PlaceholderMessage {
                        anchors.centerIn: parent
                        width: parent.width - Kirigami.Units.gridUnit * 4
                        visible: conversationListView.count === 0 && !root.conversationList.loading
                        text: root.pendingConversationFilter.length > 0
                            ? "No conversations found"
                            : "No conversations yet"
                        icon.name: "dialog-messages"
                    }

                    Controls.BusyIndicator {
                        anchors.centerIn: parent
                        running: root.conversationList.loading
                        visible: running
                    }
                }
            }
        }
    }

    // ── Conversation / message page ──────────────────────────────
    Component {
        id: conversationPageComponent

        Kirigami.Page {
            id: conversationPage
            title: root.selectedConversationName.length > 0
                ? root.selectedConversationName
                : "Messages"
            padding: 0

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                // Message list area
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    ListView {
                        id: messageList

                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.largeSpacing
                        model: root.messageListModel
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        bottomMargin: Kirigami.Units.gridUnit
                        verticalLayoutDirection: ListView.TopToBottom
                        spacing: Kirigami.Units.mediumSpacing
                        visible: !root.messageListModel.loading && root.selectedConversationIndex >= 0

                        onCountChanged: {
                            if (count > 0) {
                                positionViewAtEnd()
                            }
                            if (!root.messageListModel.loading && count > root.lastMessageCount) {
                                root.statusVisibleIndex = count - 1
                            }
                            root.lastMessageCount = count
                        }

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        delegate: messageDelegateComponent
                    }

                    Controls.BusyIndicator {
                        anchors.centerIn: parent
                        running: root.messageListModel.loading
                        visible: running
                    }

                    Kirigami.PlaceholderMessage {
                        anchors.centerIn: parent
                        width: parent.width - Kirigami.Units.gridUnit * 4
                        visible: root.selectedConversationIndex < 0 && !root.messageListModel.loading
                        text: "Select a conversation"
                        explanation: "Choose a conversation from the list to view messages."
                        icon.name: "dialog-messages"
                    }
                }

                // Separator above the send bar
                Kirigami.Separator {
                    Layout.fillWidth: true
                    visible: root.selectedConversationIndex >= 0
                }

                // Send bar
                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.largeSpacing
                    visible: root.selectedConversationIndex >= 0 && !root.messageListModel.loading

                    Controls.TextField {
                        id: sendField
                        placeholderText: "Type a message…"
                        Layout.fillWidth: true
                        text: root.outgoingText
                        onTextChanged: root.outgoingText = text
                        onAccepted: {
                            if (root.outgoingText.trim().length > 0) {
                                root.messageListModel.send_message(root.outgoingText)
                                root.outgoingText = ""
                            }
                        }
                    }
                    Controls.Button {
                        icon.name: "document-send"
                        text: "Send"
                        enabled: root.outgoingText.trim().length > 0
                        onClicked: {
                            root.messageListModel.send_message(root.outgoingText)
                            root.outgoingText = ""
                            sendField.forceActiveFocus()
                        }
                    }
                }
            }
        }
    }

    // ── Message bubble delegate ──────────────────────────────────
    Component {
        id: messageDelegateComponent

        Item {
            id: messageDelegate

            required property int index
            required property string body
            required property bool from_me
            required property string time
            required property string status

            width: ListView.view ? ListView.view.width : 0
            height: messageCol.implicitHeight

            ColumnLayout {
                id: messageCol
                width: parent.width
                spacing: Kirigami.Units.smallSpacing

                // Bubble row
                RowLayout {
                    Layout.fillWidth: true
                    layoutDirection: messageDelegate.from_me ? Qt.RightToLeft : Qt.LeftToRight
                    spacing: Kirigami.Units.smallSpacing

                    // Spacer to limit bubble width
                    Item { Layout.fillWidth: true; Layout.maximumWidth: parent.width * 0.2 }

                    Rectangle {
                        id: bubble

                        Layout.maximumWidth: messageCol.width * 0.75
                        Layout.minimumWidth: Kirigami.Units.gridUnit * 3
                        implicitWidth: Math.min(
                            bubbleText.implicitWidth + Kirigami.Units.gridUnit * 1.5,
                            messageCol.width * 0.75
                        )
                        implicitHeight: bubbleText.implicitHeight + Kirigami.Units.gridUnit * 1
                        radius: Kirigami.Units.gridUnit * 0.5
                        color: messageDelegate.from_me
                            ? Kirigami.Theme.highlightColor
                            : Kirigami.Theme.alternateBackgroundColor
                        border.width: messageDelegate.from_me ? 0 : 1
                        border.color: messageDelegate.from_me
                            ? "transparent"
                            : Qt.rgba(
                                Kirigami.Theme.textColor.r,
                                Kirigami.Theme.textColor.g,
                                Kirigami.Theme.textColor.b,
                                0.15)

                        TextEdit {
                            id: bubbleText
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.gridUnit * 0.5
                            text: messageDelegate.body
                            color: messageDelegate.from_me
                                ? Kirigami.Theme.highlightedTextColor
                                : Kirigami.Theme.textColor
                            wrapMode: Text.WordWrap
                            readOnly: true
                            selectByMouse: true
                            selectedTextColor: messageDelegate.from_me
                                ? Kirigami.Theme.textColor
                                : Kirigami.Theme.highlightedTextColor
                            selectionColor: messageDelegate.from_me
                                ? Kirigami.Theme.backgroundColor
                                : Kirigami.Theme.highlightColor
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize
                        }

                        TapHandler {
                            acceptedButtons: Qt.LeftButton
                            onTapped: root.statusVisibleIndex = messageDelegate.index
                        }
                    }

                    Item { Layout.fillWidth: true }
                }

                // Status row (time + delivery icon)
                RowLayout {
                    Layout.alignment: messageDelegate.from_me ? Qt.AlignRight : Qt.AlignLeft
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.smallSpacing
                    visible: messageDelegate.index === root.statusVisibleIndex

                    Controls.Label {
                        text: messageDelegate.time
                        color: Kirigami.Theme.disabledTextColor
                        font: Kirigami.Theme.smallFont
                    }
                    Controls.Label {
                        text: "\u00B7"
                        color: Kirigami.Theme.disabledTextColor
                        font: Kirigami.Theme.smallFont
                        visible: messageDelegate.from_me
                    }
                    Image {
                        width: 18
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

            TapHandler {
                acceptedButtons: Qt.LeftButton
                grabPermissions: PointerHandler.CanTakeOverFromAnything
                onTapped: root.statusVisibleIndex = messageDelegate.index
            }
        }
    }

    // ── Filter debounce timer ────────────────────────────────────
    Timer {
        id: filterDebounce
        interval: 150
        repeat: false
        onTriggered: root.conversationList.apply_filter(root.pendingConversationFilter)
    }

    // ── Login dialog ─────────────────────────────────────────────
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
                appState.status_message = "Not logged in"
            } else {
                if (!sessionController.running) {
                    sessionController.start()
                }
                conversationList.load()
            }
        }
    }

    // ── Connections ──────────────────────────────────────────────
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

            if (appState.logged_in) {
                // Replace the welcome page with conversation list
                root.pageStack.replace(conversationListComponent)
            } else {
                // Clear everything and show the welcome page
                root.pageStack.clear()
                root.pageStack.push(welcomeComponent)
                root.selectedConversationIndex = -1
                root.selectedConversationName = ""
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
