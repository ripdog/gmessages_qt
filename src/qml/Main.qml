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
    property bool userAtBottom: true

    // ── Helper: send current message ─────────────────────────────
    function doSendMessage() {
        if (root.outgoingText.trim().length > 0) {
            const body = root.outgoingText;
            const convoId = root.conversationList.conversation_id(root.selectedConversationIndex);
            root.messageListModel.send_message(body);
            root.outgoingText = "";
            root.messageListModel.send_typing(false);
            typingDebounce.stop();
            
            // Optimistically update conversation list preview
            root.conversationList.update_preview(convoId, "You: " + body, Date.now() * 1000);
        }
    }

    // ── pageStack configuration ──────────────────────────────────
    pageStack {
        initialPage: appState.logged_in ? conversationListComponent : welcomeComponent
        columnView.columnResizeMode: root.pageStack.wideMode
            ? Kirigami.ColumnView.DynamicColumns
            : Kirigami.ColumnView.SingleColumn
        globalToolBar {
            style: Kirigami.ApplicationHeaderStyle.ToolBar
            canContainHandles: true
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
            Kirigami.ColumnView.interactiveResizeEnabled: true

            // Page actions (refresh + logout)
            actions: [
                Kirigami.Action {
                    icon.name: "view-refresh"
                    text: "Refresh"
                    onTriggered: root.conversationList.load()
                },
                Kirigami.Action {
                    icon.name: "system-log-out"
                    text: "Log out"
                    onTriggered: {
                        root.selectedConversationIndex = -1
                        root.selectedConversationName = ""
                        root.sessionController.stop()
                        root.appState.logout("")
                    }
                }
            ]

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
                        required property string time
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
                            root.conversationList.mark_conversation_read(convoId)

                            // Push the conversation page if not already shown
                            if (root.pageStack.depth < 2) {
                                root.pageStack.push(conversationPageComponent)
                            }
                            root.pageStack.currentIndex = 1
                        }

                        contentItem: RowLayout {
                            spacing: Kirigami.Units.largeSpacing

                            // Avatar (circular clip)
                            Rectangle {
                                Layout.preferredWidth: Kirigami.Units.gridUnit * 2.5
                                Layout.preferredHeight: Kirigami.Units.gridUnit * 2.5
                                radius: width / 2
                                color: Kirigami.Theme.alternateBackgroundColor
                                clip: true

                                Image {
                                    anchors.fill: parent
                                    source: conversationDelegate.avatar_url
                                    fillMode: Image.PreserveAspectCrop
                                    visible: conversationDelegate.avatar_url.length > 0
                                }
                                Kirigami.Icon {
                                    anchors.fill: parent
                                    anchors.margins: Kirigami.Units.smallSpacing
                                    source: conversationDelegate.is_group_chat ? "group" : "user-identity"
                                    visible: conversationDelegate.avatar_url.length === 0
                                }
                            }

                            // Name + preview + timestamp
                            ColumnLayout {
                                spacing: 0
                                Layout.fillWidth: true

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: Kirigami.Units.smallSpacing

                                    Controls.Label {
                                        text: conversationDelegate.name
                                        elide: Text.ElideRight
                                        font.weight: conversationDelegate.unread ? Font.Bold : Font.Normal
                                        textFormat: Text.PlainText
                                        Layout.fillWidth: true
                                    }

                                    Controls.Label {
                                        text: conversationDelegate.time
                                        font: Kirigami.Theme.smallFont
                                        color: conversationDelegate.unread
                                            ? Kirigami.Theme.highlightColor
                                            : Kirigami.Theme.disabledTextColor
                                        textFormat: Text.PlainText
                                        visible: conversationDelegate.time.length > 0
                                    }
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
                        anchors.rightMargin: Kirigami.Units.largeSpacing + Kirigami.Units.gridUnit
                        model: root.messageListModel
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        bottomMargin: Kirigami.Units.gridUnit
                        verticalLayoutDirection: ListView.TopToBottom
                        spacing: Kirigami.Units.mediumSpacing
                        visible: !root.messageListModel.loading && root.selectedConversationIndex >= 0

                        // Date section headers
                        section.property: "section_date"
                        section.delegate: Item {
                            required property string section
                            width: ListView.view ? ListView.view.width : 0
                            height: sectionLabel.implicitHeight + Kirigami.Units.largeSpacing * 2

                            Controls.Label {
                                id: sectionLabel
                                anchors.centerIn: parent
                                text: parent.section
                                font: Kirigami.Theme.smallFont
                                color: Kirigami.Theme.disabledTextColor

                                background: Rectangle {
                                    color: Kirigami.Theme.backgroundColor
                                    radius: height / 2
                                    x: -Kirigami.Units.largeSpacing
                                    y: -Math.round(Kirigami.Units.smallSpacing / 2)
                                    width: sectionLabel.implicitWidth + Kirigami.Units.largeSpacing * 2
                                    height: sectionLabel.implicitHeight + Kirigami.Units.smallSpacing
                                }
                            }
                        }

                        // Smart scroll: only auto-scroll when user is already at the bottom
                        onContentYChanged: {
                            root.userAtBottom = atYEnd
                        }

                        onCountChanged: {
                            if (count > 0 && root.userAtBottom) {
                                scrollTimer.restart()
                            }
                            if (!root.messageListModel.loading && count > root.lastMessageCount) {
                                root.statusVisibleIndex = count - 1
                            }
                            root.lastMessageCount = count
                        }

                        // When model resets (new conversation loaded), always scroll to bottom
                        Connections {
                            target: root.messageListModel
                            function onLoadingChanged() {
                                if (!root.messageListModel.loading && messageList.count > 0) {
                                    root.userAtBottom = true
                                    scrollTimer.restart()
                                }
                            }
                        }

                        Timer {
                            id: scrollTimer
                            interval: 100
                            repeat: false
                            onTriggered: {
                                messageList.positionViewAtEnd()
                                root.userAtBottom = true
                            }
                        }

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        delegate: messageDelegateComponent
                    }

                    // Jump-to-bottom button
                    Controls.RoundButton {
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottomMargin: Kirigami.Units.largeSpacing
                        icon.name: "go-down"
                        visible: !root.userAtBottom && messageList.visible && messageList.count > 0
                        onClicked: {
                            messageList.positionViewAtEnd()
                            root.userAtBottom = true
                        }
                        z: 1
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

                // Send bar (multi-line TextArea)
                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.largeSpacing
                    visible: root.selectedConversationIndex >= 0 && !root.messageListModel.loading

                    Controls.ScrollView {
                        Layout.fillWidth: true
                        Layout.maximumHeight: Kirigami.Units.gridUnit * 8

                        Controls.TextArea {
                            id: sendField
                            placeholderText: "Type a message…"
                            wrapMode: TextEdit.Wrap
                            text: root.outgoingText
                            onTextChanged: {
                                root.outgoingText = text
                                // Send typing indicator with debounce
                                if (text.trim().length > 0) {
                                    if (!typingDebounce.running) {
                                        root.messageListModel.send_typing(true)
                                    }
                                    typingDebounce.restart()
                                } else {
                                    root.messageListModel.send_typing(false)
                                    typingDebounce.stop()
                                }
                            }
                            Keys.onReturnPressed: function(event) {
                                if (event.modifiers & Qt.ShiftModifier) {
                                    // Shift+Enter: insert newline
                                    event.accepted = false
                                } else {
                                    // Enter: send message
                                    event.accepted = true
                                    root.doSendMessage()
                                }
                            }
                        }
                    }
                    Controls.Button {
                        icon.name: "document-send"
                        text: "Send"
                        Layout.alignment: Qt.AlignBottom
                        enabled: root.outgoingText.trim().length > 0
                        onClicked: {
                            root.doSendMessage()
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
            required property string section_date
            required property string media_url
            required property bool is_media

            width: ListView.view ? ListView.view.width : 0
            height: messageCol.implicitHeight

            readonly property bool isFailed: messageDelegate.status === "failed"

            ColumnLayout {
                id: messageCol
                width: parent.width
                spacing: Kirigami.Units.smallSpacing

                // Bubble row with avatar
                RowLayout {
                    Layout.fillWidth: true
                    layoutDirection: messageDelegate.from_me ? Qt.RightToLeft : Qt.LeftToRight
                    spacing: Kirigami.Units.smallSpacing

                    // Avatar circle
                    Rectangle {
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 1.5
                        Layout.preferredHeight: Kirigami.Units.gridUnit * 1.5
                        Layout.alignment: Qt.AlignBottom
                        radius: width / 2
                        color: messageDelegate.from_me
                            ? Kirigami.Theme.highlightColor
                            : Kirigami.Theme.disabledTextColor

                        Controls.Label {
                            anchors.centerIn: parent
                            text: messageDelegate.from_me ? "Me" : "?"
                            color: "white"
                            font.pixelSize: Math.round(parent.height * 0.45)
                            font.bold: true
                        }
                    }

                    Rectangle {
                        id: bubble

                        Layout.maximumWidth: messageCol.width * 0.75
                        Layout.minimumWidth: Kirigami.Units.gridUnit * 3
                        implicitWidth: Math.min(
                            bubbleContent.implicitWidth + Kirigami.Units.gridUnit * 1.5,
                            messageCol.width * 0.75
                        )
                        implicitHeight: bubbleContent.implicitHeight + Kirigami.Units.gridUnit * 1
                        radius: Kirigami.Units.gridUnit * 0.5
                        color: messageDelegate.isFailed
                            ? Qt.rgba(Kirigami.Theme.negativeTextColor.r,
                                      Kirigami.Theme.negativeTextColor.g,
                                      Kirigami.Theme.negativeTextColor.b, 0.15)
                            : messageDelegate.from_me
                                ? Kirigami.Theme.highlightColor
                                : Kirigami.Theme.alternateBackgroundColor
                        border.width: messageDelegate.isFailed ? 1
                            : messageDelegate.from_me ? 0 : 1
                        border.color: messageDelegate.isFailed
                            ? Kirigami.Theme.negativeTextColor
                            : messageDelegate.from_me
                                ? "transparent"
                                : Qt.rgba(
                                    Kirigami.Theme.textColor.r,
                                    Kirigami.Theme.textColor.g,
                                    Kirigami.Theme.textColor.b,
                                    0.15)

                        ColumnLayout {
                            id: bubbleContent
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.gridUnit * 0.5
                            spacing: Kirigami.Units.smallSpacing

                            Image {
                                Layout.maximumWidth: messageCol.width * 0.6
                                Layout.maximumHeight: Kirigami.Units.gridUnit * 15
                                Layout.alignment: Qt.AlignHCenter
                                Layout.margins: 0
                                fillMode: Image.PreserveAspectFit
                                source: messageDelegate.media_url
                                visible: messageDelegate.is_media && messageDelegate.media_url.length > 0
                            }

                            Controls.BusyIndicator {
                                Layout.alignment: Qt.AlignHCenter
                                visible: messageDelegate.is_media && messageDelegate.media_url.length === 0
                            }

                            TextEdit {
                                id: bubbleText
                                Layout.fillWidth: true
                                text: messageDelegate.body
                                color: messageDelegate.isFailed
                                    ? Kirigami.Theme.negativeTextColor
                                    : messageDelegate.from_me
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
                                visible: messageDelegate.body.length > 0
                            }
                        }

                        TapHandler {
                            acceptedButtons: Qt.LeftButton
                            onTapped: root.statusVisibleIndex = messageDelegate.index
                        }
                    }

                    // Fill remaining space to push bubble to the correct side
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
                    // Failed status: show error text instead of icon
                    Controls.Label {
                        text: "Failed to send"
                        color: Kirigami.Theme.negativeTextColor
                        font: Kirigami.Theme.smallFont
                        visible: messageDelegate.isFailed
                    }
                    // Normal status icon
                    Image {
                        width: 18
                        height: 12
                        fillMode: Image.PreserveAspectFit
                        visible: messageDelegate.from_me && !messageDelegate.isFailed
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

    // ── Typing indicator debounce timer ──────────────────────────
    Timer {
        id: typingDebounce
        interval: 5000
        repeat: false
        onTriggered: root.messageListModel.send_typing(false)
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
                root.showPassiveNotification("Logged out", "short")
            }
        }
    }

    Connections {
        target: sessionController

        function onMessage_received(conversationId, participantId, body, transportType, messageId, tmpId, timestampMicros, statusCode, isMedia, mediaId, decryptionKey, mimeType) {
            messageListModel.handle_message_event(conversationId, participantId, body, transportType, messageId, tmpId, timestampMicros, statusCode, isMedia)
            conversationList.update_preview(conversationId, isMedia ? "Media" : body, timestampMicros)
            
            if (isMedia && mediaId.length > 0) {
                messageListModel.queue_media_download(messageId.length > 0 ? messageId : tmpId, mediaId, decryptionKey, mimeType)
            }
        }

        function onConversation_updated(conversationId, name, preview, unread, lastMessageTimestamp, isGroupChat) {
            conversationList.handle_conversation_event(conversationId, name, preview, unread, lastMessageTimestamp, isGroupChat)
        }
    }

    Connections {
        target: conversationList

        function onAuth_error(message) {
            root.showPassiveNotification("Authentication error: " + message, "long")
            appState.logout(message)
        }
    }

    Connections {
        target: messageListModel

        function onAuth_error(message) {
            root.showPassiveNotification("Authentication error: " + message, "long")
            appState.logout(message)
        }
    }
}
