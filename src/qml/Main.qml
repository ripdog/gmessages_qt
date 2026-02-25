import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.delegates as Delegates
import QtQuick.Dialogs
import org.kourier
import Qt.labs.platform as Platform

Kirigami.ApplicationWindow {
    id: root

    property bool actuallyQuit: false
    property var notifiedTimestamps: ({})
    property bool startBackground: Qt.application.arguments.indexOf("--background") !== -1 || Qt.application.arguments.indexOf("--invisible") !== -1

    visible: false
    onClosing: function(close) {
        if (!actuallyQuit) {
            close.accepted = false
            root.visible = false
        }
    }

    onActiveChanged: {
        if (active && root.selectedConversationIndex >= 0) {
            const convoId = root.conversationList.conversation_id(root.selectedConversationIndex);
            root.messageListModel.mark_latest_as_read();
            root.conversationList.mark_conversation_read(convoId);
        }
    }

    Platform.SystemTrayIcon {
        id: trayIcon
        visible: true
        icon.source: "qrc:/svg/kourier.svg"
        tooltip: "Kourier"

        menu: Platform.Menu {
            Platform.MenuItem {
                text: root.visible ? "Hide Window" : "Show Window"
                onTriggered: {
                    root.visible = !root.visible
                    if (root.visible) {
                        root.requestActivate()
                    }
                }
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: "Quit"
                onTriggered: {
                    root.actuallyQuit = true
                    Qt.quit()
                }
            }
        }

        onActivated: function(reason) {
            if (reason === Platform.SystemTrayIcon.Trigger) {
                root.visible = !root.visible
                if (root.visible) {
                    root.requestActivate()
                }
            }
        }
    }

    title: selectedConversationName.length > 0 ? selectedConversationName + " — Kourier" : "Kourier"

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

    // ── Staged attachments model ───────────────────────────────────
    ListModel {
        id: stagedAttachments
    }

    // ── Helper: send current message ─────────────────────────────
    function doSendMessage() {
        const hasText = root.outgoingText.trim().length > 0;
        const hasMedia = stagedAttachments.count > 0;

        if (!hasText && !hasMedia) return;

        const body = root.outgoingText.trim();
        const convoId = root.conversationList.conversation_id(root.selectedConversationIndex);

        if (hasMedia) {
            // Send each attachment individually; text goes with the last one
            for (let i = 0; i < stagedAttachments.count; i++) {
                const fileUrl = stagedAttachments.get(i).fileUrl;
                const caption = (i === stagedAttachments.count - 1 && hasText) ? body : "";
                root.messageListModel.send_media(fileUrl, caption);
            }
            stagedAttachments.clear();
            root.conversationList.update_preview(convoId, hasText ? "You: " + body : "You: Media", Date.now() * 1000);
        } else {
            root.messageListModel.send_message(body);
            root.conversationList.update_preview(convoId, "You: " + body, Date.now() * 1000);
        }

        root.outgoingText = "";
        root.messageListModel.send_typing(false);
        typingDebounce.stop();
    }

    // ── Global Drawer ────────────────────────────────────────────
    globalDrawer: Kirigami.GlobalDrawer {
        id: globalDrawer
        
        // The drawer needs at least one action or it might refuse to open
        actions: [
            Kirigami.Action {
                text: "Kourier"
                enabled: false
            }
        ]

        footer: ColumnLayout {
            spacing: 0
            
            Kirigami.Separator {
                Layout.fillWidth: true
            }

            Controls.ItemDelegate {
                Layout.fillWidth: true
                text: "Refresh"
                icon.name: "view-refresh"
                onClicked: {
                    globalDrawer.close()
                    root.conversationList.load()
                }
            }
            Controls.ItemDelegate {
                Layout.fillWidth: true
                text: "Clear Cache"
                icon.name: "edit-clear"
                onClicked: {
                    globalDrawer.close()
                    root.appState.clear_cache()
                    root.showPassiveNotification("Cache cleared", "short")
                }
            }
            Controls.ItemDelegate {
                Layout.fillWidth: true
                text: "Log out"
                icon.name: "system-log-out"
                onClicked: {
                    globalDrawer.close()
                    root.selectedConversationIndex = -1
                    root.selectedConversationName = ""
                    root.sessionController.stop()
                    root.appState.logout("")
                }
            }
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
            title: "Welcome"

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.gridUnit * 4

                icon.source: "qrc:/svg/kourier.svg"
                text: "Welcome to Kourier"
                explanation: "An unofficial native KDE client for Google Messages."

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

                    onAtYEndChanged: {
                        if (atYEnd && count > 0 && !root.conversationList.loading) {
                            root.conversationList.load_more()
                        }
                    }

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
                        verticalLayoutDirection: ListView.BottomToTop
                        spacing: Kirigami.Units.mediumSpacing
                        reuseItems: true
                        visible: !root.messageListModel.loading && root.selectedConversationIndex >= 0

                        // ── Load-more state ──
                        property int _preLoadCount: 0
                        property bool _awaitingLoadMore: false
                        property bool _suppressLoadMore: false

                        // ── BottomToTop coordinate mapping (empirically verified) ──
                        // atYEnd     = visual BOTTOM (newest messages, contentY ≈ -height)
                        // atYBeginning = visual TOP (oldest messages, contentY ≈ originY)

                        // ── Bottom tracking ──
                        // Only set userAtBottom=FALSE here (safe: just shows button).
                        // Setting TRUE from contentY changes causes feedback loops with media loading.
                        onContentYChanged: {
                            if (!atYEnd) {
                                root.userAtBottom = false
                            }
                        }

                        // userAtBottom=TRUE only on user-initiated scroll completion
                        onMovementEnded: {
                            root.userAtBottom = atYEnd

                            // Trigger load_more when scrolled to visual top (oldest messages)
                            if (atYBeginning && !atYEnd && !_awaitingLoadMore && !_suppressLoadMore
                                    && count > 0 && !root.messageListModel.loading && visible) {
                                _preLoadCount = count
                                _awaitingLoadMore = true
                                root.messageListModel.load_more()
                            }
                        }

                        onCountChanged: {
                            // After load_more: Qt preserves viewport for appended items
                            // via begin_insert_rows, so we just clear the flag
                            if (_awaitingLoadMore && count > _preLoadCount && _preLoadCount > 0) {
                                _awaitingLoadMore = false
                                _preLoadCount = 0
                                return
                            }

                            // Auto-scroll for new incoming messages only if at bottom
                            if (count > 0 && root.userAtBottom && !_awaitingLoadMore) {
                                scrollTimer.restart()
                            }
                            if (!root.messageListModel.loading && count > root.lastMessageCount) {
                                root.statusVisibleIndex = 0
                            }
                            root.lastMessageCount = count
                        }

                        // When conversation loads, scroll to bottom and suppress load_more
                        Connections {
                            target: root.messageListModel
                            function onLoadingChanged() {
                                if (!root.messageListModel.loading && messageList.count > 0) {
                                    root.userAtBottom = true
                                    messageList._awaitingLoadMore = false
                                    messageList._suppressLoadMore = true
                                    suppressTimer.restart()
                                    scrollTimer.restart()
                                }
                            }
                        }

                        // Suppress load_more for 1s after conversation load
                        Timer {
                            id: suppressTimer
                            interval: 1000
                            onTriggered: messageList._suppressLoadMore = false
                        }

                        Timer {
                            id: scrollTimer
                            interval: 50
                            repeat: false
                            onTriggered: {
                                messageList.positionViewAtBeginning()
                                root.userAtBottom = true
                            }
                        }

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        delegate: MessageDelegate {}
                    }

                    // Loading indicator for older messages
                    Controls.BusyIndicator {
                        anchors.top: parent.top
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.topMargin: Kirigami.Units.largeSpacing
                        running: root.messageListModel.loading_more
                        visible: running
                        z: 1
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

                // ── Staged attachments preview ──────────────────────
                Flow {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Layout.topMargin: stagedAttachments.count > 0 ? Kirigami.Units.smallSpacing : 0
                    spacing: Kirigami.Units.smallSpacing
                    visible: stagedAttachments.count > 0 && root.selectedConversationIndex >= 0

                    Repeater {
                        model: stagedAttachments

                        Rectangle {
                            id: thumbContainer
                            required property int index
                            required property string fileUrl
                            required property string thumbUrl

                            readonly property bool isVideoFile: {
                                const lower = thumbContainer.fileUrl.toLowerCase()
                                return lower.endsWith(".mp4") || lower.endsWith(".webm")
                                    || lower.endsWith(".3gp") || lower.endsWith(".3g2")
                            }

                            width: Kirigami.Units.gridUnit * 5
                            height: Kirigami.Units.gridUnit * 5
                            radius: Kirigami.Units.smallSpacing
                            color: Kirigami.Theme.alternateBackgroundColor
                            clip: true

                            // Image thumbnail
                            Image {
                                anchors.fill: parent
                                source: thumbContainer.isVideoFile ? thumbContainer.thumbUrl : thumbContainer.fileUrl
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                sourceSize.width: Kirigami.Units.gridUnit * 5
                                sourceSize.height: Kirigami.Units.gridUnit * 5
                                visible: source.toString().length > 0
                            }

                            // Video play button overlay
                            Item {
                                anchors.fill: parent
                                visible: thumbContainer.isVideoFile

                                Rectangle {
                                    anchors.fill: parent
                                    color: Qt.rgba(0, 0, 0, 0.3)
                                }

                                Kirigami.Icon {
                                    anchors.centerIn: parent
                                    width: Kirigami.Units.iconSizes.medium
                                    height: Kirigami.Units.iconSizes.medium
                                    source: "media-playback-start"
                                    color: "white"
                                }
                            }

                            // Semi-transparent scrim behind the X
                            Rectangle {
                                anchors.top: parent.top
                                anchors.right: parent.right
                                width: Kirigami.Units.gridUnit * 1.5
                                height: Kirigami.Units.gridUnit * 1.5
                                radius: width / 2
                                color: Qt.rgba(0, 0, 0, 0.55)

                                Kirigami.Icon {
                                    anchors.centerIn: parent
                                    width: Kirigami.Units.iconSizes.small
                                    height: Kirigami.Units.iconSizes.small
                                    source: "dialog-close"
                                    color: "white"
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: stagedAttachments.remove(thumbContainer.index)
                                }
                            }
                        }
                    }
                }

                // Send bar (multi-line TextArea)
                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.largeSpacing
                    visible: root.selectedConversationIndex >= 0 && !root.messageListModel.loading

                    Controls.RoundButton {
                        icon.name: "list-add"
                        Layout.alignment: Qt.AlignBottom
                        onClicked: attachmentDialog.open()
                        Controls.ToolTip.text: "Add media"
                        Controls.ToolTip.visible: hovered
                        Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                    }

                    Controls.ScrollView {
                        Layout.fillWidth: true
                        Layout.maximumHeight: Kirigami.Units.gridUnit * 8

                        Controls.TextArea {
                            id: sendField
                            placeholderText: stagedAttachments.count > 0
                                ? "Add a caption…"
                                : "Type a message…"
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
                        enabled: root.outgoingText.trim().length > 0 || stagedAttachments.count > 0
                        onClicked: {
                            root.doSendMessage()
                            sendField.forceActiveFocus()
                        }
                    }
                }
            }
        }
    }

    // ── Message bubble delegate extracted to MessageDelegate.qml ──

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

    // ── Login dialog ──
    LoginDialog {
        id: loginDialog
    }

    // ── Media Viewer Dialog ──
    MediaViewerDialog {
        id: mediaViewerDialog
    }

    // ── Attachment Dialog ──
    FileDialog {
        id: attachmentDialog
        title: "Select Attachment"
        nameFilters: ["All media (*.png *.jpg *.jpeg *.gif *.webp *.mp4 *.webm *.3gp)", "Images (*.png *.jpg *.jpeg *.gif *.webp)", "Videos (*.mp4 *.webm *.3gp)", "All files (*)"]
        onAccepted: {
            const url = String(attachmentDialog.selectedFile || attachmentDialog.currentFile);
            if (url.length > 0) {
                // Check file size via Rust FFI (fs::metadata — instant, no file read)
                const maxBytes = 100 * 1024 * 1024; // 100 MB RCS limit
                const size = root.messageListModel.get_file_size(url);
                if (size < 0) {
                    root.showPassiveNotification("Could not read file info", "short");
                    return;
                }
                if (size > maxBytes) {
                    const sizeMB = (size / 1048576).toFixed(1);
                    root.showPassiveNotification(
                        `File too large: ${sizeMB} MB (RCS limit is 100 MB)`,
                        "long"
                    );
                    return;
                }

                let thumb = "";
                const lower = url.toLowerCase();
                if (lower.endsWith(".mp4") || lower.endsWith(".webm") || lower.endsWith(".3gp") || lower.endsWith(".3g2")) {
                    thumb = root.messageListModel.get_video_thumbnail(url);
                }
                stagedAttachments.append({ fileUrl: url, thumbUrl: thumb });
            }
        }
    }

    // ── Connections ──────────────────────────────────────────────
    Connections {
        target: appState

        function onInitialized(loggedIn) {
            if (!loggedIn) {
                root.visible = true
            } else if (!startBackground) {
                root.visible = true
            }
        }

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

            if (root.active && root.selectedConversationIndex >= 0 && root.conversationList.conversation_id(root.selectedConversationIndex) === conversationId) {
                root.messageListModel.mark_latest_as_read();
                root.conversationList.mark_conversation_read(conversationId);
            }
        }

        function onConversation_updated(conversationId, name, preview, unread, lastMessageTimestamp, isGroupChat) {
            conversationList.handle_conversation_event(conversationId, name, preview, unread, lastMessageTimestamp, isGroupChat)

            if (unread && lastMessageTimestamp > Date.now() * 1000 - 60000000) {
                const isViewing = (root.selectedConversationIndex >= 0 && root.conversationList.conversation_id(root.selectedConversationIndex) === conversationId)
                
                if (!(isViewing && root.active)) {
                    const lastNotified = root.notifiedTimestamps[conversationId] || 0
                    if (lastMessageTimestamp > lastNotified) {
                        root.notifiedTimestamps[conversationId] = lastMessageTimestamp
                        trayIcon.showMessage(name, preview, Platform.SystemTrayIcon.Information, 5000)
                    }
                }
            }
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
