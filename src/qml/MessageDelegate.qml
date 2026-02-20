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

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
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
