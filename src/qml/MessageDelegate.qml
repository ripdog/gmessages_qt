import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

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
    required property string avatar_url
    required property bool is_info
    required property int transport_type
    required property string mime_type
    required property string message_id
    required property string thumbnail_url
    required property real upload_progress
    required property string link_url
    required property string link_title
    required property string link_image_url
    required property int media_width
    required property int media_height

    required property bool is_start_of_day

    width: ListView.view ? ListView.view.width : 0
    height: messageCol.implicitHeight

    readonly property bool isFailed: messageDelegate.status === "failed"
    readonly property bool isSending: messageDelegate.status === "sending"
    // 1=SMS, 2=Downloaded MMS, 3=Undownloaded MMS
    readonly property bool isSms: messageDelegate.transport_type === 1 || messageDelegate.transport_type === 2 || messageDelegate.transport_type === 3
    readonly property bool isVideo: messageDelegate.mime_type.startsWith("video/")
    readonly property bool hasLinkPreview: messageDelegate.link_title.length > 0

    // Convert plain-text body into HTML with clickable links
    function linkifyBody(text) {
        // Escape HTML entities first
        let escaped = text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
        // Replace URLs with anchor tags
        escaped = escaped.replace(/(https?:\/\/[^\s<]+)/g, '<a href="$1">$1</a>');
        // Convert newlines to <br>
        escaped = escaped.replace(/\n/g, '<br>');
        return escaped;
    }

    ColumnLayout {
        id: messageCol
        width: parent.width
        spacing: Kirigami.Units.smallSpacing

        // Date header
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: sectionLabel.implicitHeight + Kirigami.Units.largeSpacing * 2
            visible: messageDelegate.is_start_of_day

            Controls.Label {
                id: sectionLabel
                anchors.centerIn: parent
                text: messageDelegate.section_date
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

        // info message
        Controls.Label {
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            visible: messageDelegate.is_info
            text: messageDelegate.body
            color: Kirigami.Theme.disabledTextColor
            font.family: Kirigami.Theme.smallFont.family
            font.pointSize: Kirigami.Theme.smallFont.pointSize + 1
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            Layout.maximumWidth: parent.width * 0.8
        }

        // Bubble row with avatar
        RowLayout {
            Layout.fillWidth: true
            layoutDirection: messageDelegate.from_me ? Qt.RightToLeft : Qt.LeftToRight
            spacing: Kirigami.Units.smallSpacing
            visible: !messageDelegate.is_info

            // Avatar circle
            Rectangle {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 1.5
                Layout.preferredHeight: Kirigami.Units.gridUnit * 1.5
                Layout.alignment: Qt.AlignBottom
                radius: width / 2
                color: messageDelegate.from_me
                    ? Kirigami.Theme.highlightColor
                    : Kirigami.Theme.disabledTextColor
                clip: true

                Image {
                    anchors.fill: parent
                    source: messageDelegate.avatar_url
                    fillMode: Image.PreserveAspectCrop
                    visible: !messageDelegate.from_me && messageDelegate.avatar_url.length > 0
                }

                Controls.Label {
                    anchors.centerIn: parent
                    text: messageDelegate.from_me ? "Me" : "?"
                    color: "white"
                    font.pixelSize: Math.round(parent.height * 0.45)
                    font.bold: true
                    visible: messageDelegate.from_me || messageDelegate.avatar_url.length === 0
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
                
                // Diff color for SMS vs RCS if sent by us
                color: messageDelegate.isFailed
                    ? Qt.rgba(Kirigami.Theme.negativeTextColor.r,
                              Kirigami.Theme.negativeTextColor.g,
                              Kirigami.Theme.negativeTextColor.b, 0.15)
                    : messageDelegate.from_me
                        ? (messageDelegate.isSms ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.highlightColor)
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

                    // ── Image media ──
                    Image {
                        id: mediaImage
                        // Calculate stable height from server-provided dimensions
                        readonly property real maxW: messageCol.width * 0.6
                        readonly property real maxH: Kirigami.Units.gridUnit * 15
                        readonly property bool hasDimensions: messageDelegate.media_width > 0 && messageDelegate.media_height > 0
                        readonly property real scaledHeight: hasDimensions
                            ? Math.min(maxH, (messageDelegate.media_height / messageDelegate.media_width) * Math.min(maxW, messageDelegate.media_width))
                            : -1

                        Layout.maximumWidth: maxW
                        Layout.maximumHeight: maxH
                        Layout.preferredHeight: hasDimensions ? scaledHeight : -1
                        Layout.preferredWidth: hasDimensions
                            ? Math.min(maxW, messageDelegate.media_width * (scaledHeight / messageDelegate.media_height))
                            : -1
                        Layout.alignment: Qt.AlignHCenter
                        Layout.margins: 0
                        fillMode: Image.PreserveAspectFit
                        source: messageDelegate.isVideo ? "" : messageDelegate.media_url
                        visible: messageDelegate.is_media && messageDelegate.media_url.length > 0 && !messageDelegate.isVideo
                        sourceSize.width: 400
                        sourceSize.height: 400
                        asynchronous: true

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                mediaViewerDialog.mimeType = messageDelegate.mime_type
                                mediaViewerDialog.sourceUrl = messageDelegate.media_url
                                mediaViewerDialog.isActualSize = false
                                mediaViewerDialog.open()
                            }
                        }

                        Controls.ProgressBar {
                            anchors.bottom: parent.bottom
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.margins: Kirigami.Units.smallSpacing
                            from: 0
                            to: 1
                            value: messageDelegate.upload_progress
                            indeterminate: messageDelegate.upload_progress === 0.0
                            visible: messageDelegate.isSending && messageDelegate.upload_progress < 1.0
                        }

                        // Cancel upload button
                        Rectangle {
                            anchors.top: parent.top
                            anchors.right: parent.right
                            anchors.margins: Kirigami.Units.smallSpacing
                            width: Kirigami.Units.gridUnit * 1.5
                            height: Kirigami.Units.gridUnit * 1.5
                            radius: width / 2
                            color: Qt.rgba(0.8, 0.1, 0.1, 0.85)
                            visible: messageDelegate.isSending

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
                                onClicked: messageListModel.delete_message(messageDelegate.message_id)
                            }
                        }
                    }

                    // ── Video media (thumbnail with play overlay) ──
                    Item {
                        Layout.maximumWidth: messageCol.width * 0.6
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                        Layout.preferredHeight: Kirigami.Units.gridUnit * 9
                        Layout.alignment: Qt.AlignHCenter
                        visible: messageDelegate.is_media && messageDelegate.media_url.length > 0 && messageDelegate.isVideo

                        Rectangle {
                            anchors.fill: parent
                            color: Qt.rgba(0, 0, 0, 0.3)
                            radius: Kirigami.Units.smallSpacing
                            clip: true

                            Image {
                                anchors.fill: parent
                                source: messageDelegate.thumbnail_url
                                fillMode: Image.PreserveAspectCrop
                                visible: messageDelegate.thumbnail_url.length > 0
                            }

                            // Dark overlay to make the play button and text readable
                            Rectangle {
                                anchors.fill: parent
                                color: Qt.rgba(0, 0, 0, 0.3)
                            }

                            // Play button overlay
                            Rectangle {
                                anchors.centerIn: parent
                                width: Kirigami.Units.gridUnit * 3
                                height: Kirigami.Units.gridUnit * 3
                                radius: width / 2
                                color: Qt.rgba(0, 0, 0, 0.6)

                                Kirigami.Icon {
                                    anchors.centerIn: parent
                                    width: Kirigami.Units.iconSizes.medium
                                    height: Kirigami.Units.iconSizes.medium
                                    source: "media-playback-start"
                                    color: "white"
                                }
                            }

                            // "Video" label
                            Controls.Label {
                                anchors.bottom: parent.bottom
                                anchors.left: parent.left
                                anchors.margins: Kirigami.Units.smallSpacing
                                text: "Video"
                                color: "white"
                                font: Kirigami.Theme.smallFont
                            }

                            Controls.ProgressBar {
                                anchors.bottom: parent.bottom
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.margins: Kirigami.Units.smallSpacing
                                from: 0
                                to: 1
                                value: messageDelegate.upload_progress
                                indeterminate: messageDelegate.upload_progress === 0.0
                                visible: messageDelegate.isSending && messageDelegate.upload_progress < 1.0
                            }

                            // Cancel upload button
                            Rectangle {
                                anchors.top: parent.top
                                anchors.right: parent.right
                                anchors.margins: Kirigami.Units.smallSpacing
                                width: Kirigami.Units.gridUnit * 1.5
                                height: Kirigami.Units.gridUnit * 1.5
                                radius: width / 2
                                color: Qt.rgba(0.8, 0.1, 0.1, 0.85)
                                visible: messageDelegate.isSending

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
                                    onClicked: messageListModel.delete_message(messageDelegate.message_id)
                                }
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                mediaViewerDialog.mimeType = messageDelegate.mime_type
                                mediaViewerDialog.sourceUrl = messageDelegate.media_url
                                mediaViewerDialog.isActualSize = false
                                mediaViewerDialog.open()
                            }
                        }
                    }

                    Controls.BusyIndicator {
                        Layout.alignment: Qt.AlignHCenter
                        visible: messageDelegate.is_media && messageDelegate.media_url.length === 0
                    }

                    // ── Message body text with clickable links ──
                    TextEdit {
                        id: bubbleText
                        Layout.fillWidth: true
                        text: messageDelegate.linkifyBody(messageDelegate.body)
                        textFormat: Text.RichText
                        color: messageDelegate.isFailed
                            ? Kirigami.Theme.negativeTextColor
                            : messageDelegate.from_me
                                ? Kirigami.Theme.highlightedTextColor // white usually
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
                        onLinkActivated: function(link) {
                            Qt.openUrlExternally(link)
                        }

                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.NoButton
                            cursorShape: parent.hoveredLink.length > 0 ? Qt.PointingHandCursor : Qt.IBeamCursor
                        }
                    }

                    // ── Link preview card ──
                    Rectangle {
                        id: linkPreviewCard
                        Layout.fillWidth: true
                        Layout.topMargin: Kirigami.Units.smallSpacing
                        visible: messageDelegate.hasLinkPreview
                        implicitHeight: linkPreviewColumn.implicitHeight
                        radius: Kirigami.Units.smallSpacing
                        color: messageDelegate.from_me
                            ? Qt.rgba(0, 0, 0, 0.15)
                            : Qt.rgba(Kirigami.Theme.textColor.r,
                                      Kirigami.Theme.textColor.g,
                                      Kirigami.Theme.textColor.b, 0.06)
                        border.width: messageDelegate.from_me ? 0 : 1
                        border.color: Qt.rgba(Kirigami.Theme.textColor.r,
                                              Kirigami.Theme.textColor.g,
                                              Kirigami.Theme.textColor.b, 0.1)
                        clip: true

                        ColumnLayout {
                            id: linkPreviewColumn
                            anchors.left: parent.left
                            anchors.right: parent.right
                            spacing: 0

                            // Preview image
                            Image {
                                id: linkPreviewImage
                                Layout.fillWidth: true
                                Layout.maximumHeight: Kirigami.Units.gridUnit * 10
                                Layout.minimumHeight: Kirigami.Units.gridUnit * 4
                                fillMode: Image.PreserveAspectCrop
                                source: messageDelegate.link_image_url
                                visible: messageDelegate.link_image_url.length > 0 && status === Image.Ready
                                asynchronous: true
                                sourceSize.width: 600
                            }

                            // Title + domain row
                            ColumnLayout {
                                Layout.fillWidth: true
                                Layout.margins: Kirigami.Units.smallSpacing * 1.5
                                spacing: Kirigami.Units.smallSpacing * 0.5

                                Controls.Label {
                                    Layout.fillWidth: true
                                    text: messageDelegate.link_title
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: 3
                                    elide: Text.ElideRight
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize - 1
                                    font.weight: Font.Medium
                                    color: messageDelegate.from_me
                                        ? Kirigami.Theme.highlightedTextColor
                                        : Kirigami.Theme.textColor
                                    visible: messageDelegate.link_title.length > 0
                                }

                                Controls.Label {
                                    Layout.fillWidth: true
                                    text: {
                                        try {
                                            const url = new URL(messageDelegate.link_url);
                                            return url.hostname;
                                        } catch (e) {
                                            return messageDelegate.link_url;
                                        }
                                    }
                                    elide: Text.ElideRight
                                    font: Kirigami.Theme.smallFont
                                    color: messageDelegate.from_me
                                        ? Qt.rgba(Kirigami.Theme.highlightedTextColor.r,
                                                  Kirigami.Theme.highlightedTextColor.g,
                                                  Kirigami.Theme.highlightedTextColor.b, 0.7)
                                        : Kirigami.Theme.disabledTextColor
                                    visible: messageDelegate.link_url.length > 0
                                }
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Qt.openUrlExternally(messageDelegate.link_url)
                        }
                    }
                }

                TapHandler {
                    acceptedButtons: Qt.LeftButton
                    onTapped: root.statusVisibleIndex = messageDelegate.index
                }

                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: messageContextMenu.popup()
                }

                Controls.Menu {
                    id: messageContextMenu

                    Controls.MenuItem {
                        text: "Delete message"
                        icon.name: "edit-delete"
                        enabled: messageDelegate.from_me && !messageDelegate.is_info
                        onTriggered: {
                            root.messageListModel.delete_message(messageDelegate.message_id)
                        }
                    }
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
            visible: !messageDelegate.is_info && messageDelegate.index === root.statusVisibleIndex

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
