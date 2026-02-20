import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import QtMultimedia
import org.kde.kirigami as Kirigami

Controls.Dialog {
    id: mediaViewerDialog
    
    modal: true
    parent: Controls.Overlay.overlay
    anchors.centerIn: parent
    width: root.width * 0.95
    height: root.height * 0.95
    padding: 0
    margins: 0
    
    property string sourceUrl: ""
    property string mimeType: ""
    property bool isActualSize: false
    property bool windowTooBig: viewerContent.width >= imageActual.implicitWidth && viewerContent.height >= imageActual.implicitHeight

    readonly property bool isVideo: mimeType.startsWith("video/")
    
    onClosed: {
        if (videoPlayer.playbackState === MediaPlayer.PlayingState) {
            videoPlayer.stop()
        }
        sourceUrl = ""
        mimeType = ""
        isActualSize = false
    }
    
    background: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.9)
        radius: Kirigami.Units.smallSpacing
    }
    
    contentItem: Item {
        id: viewerContent
        anchors.fill: parent
        clip: true

        // ── Image viewer ──
        Flickable {
            id: imageFlickable
            anchors.fill: parent
            contentWidth: mediaViewerDialog.isActualSize ? imageActual.implicitWidth : width
            contentHeight: mediaViewerDialog.isActualSize ? imageActual.implicitHeight : height
            interactive: mediaViewerDialog.isActualSize && !mediaViewerDialog.windowTooBig
            visible: !mediaViewerDialog.isVideo
            
            Image {
                id: imageActual
                source: mediaViewerDialog.isVideo ? "" : mediaViewerDialog.sourceUrl
                fillMode: mediaViewerDialog.isActualSize ? Image.Pad : Image.PreserveAspectFit
                width: mediaViewerDialog.isActualSize ? implicitWidth : imageFlickable.width
                height: mediaViewerDialog.isActualSize ? implicitHeight : imageFlickable.height
                
                x: Math.max(0, (imageFlickable.width - width) / 2)
                y: Math.max(0, (imageFlickable.height - height) / 2)
            }
        }
        
        // ── Image zoom mouse area ──
        MouseArea {
            id: mediaMouseArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: mediaViewerDialog.windowTooBig ? Qt.ArrowCursor : Qt.BlankCursor
            visible: !mediaViewerDialog.isVideo
            
            onClicked: {
                if (!mediaViewerDialog.windowTooBig) {
                    mediaViewerDialog.isActualSize = !mediaViewerDialog.isActualSize
                    if (mediaViewerDialog.isActualSize) {
                        imageFlickable.contentX = Math.max(0, (imageFlickable.contentWidth - imageFlickable.width) / 2)
                        imageFlickable.contentY = Math.max(0, (imageFlickable.contentHeight - imageFlickable.height) / 2)
                    } else {
                        imageFlickable.contentX = 0
                        imageFlickable.contentY = 0
                    }
                }
            }
            
            // Custom zoom cursor
            Rectangle {
                width: Kirigami.Units.iconSizes.medium + Kirigami.Units.smallSpacing * 2
                height: width
                radius: width / 2
                color: Kirigami.Theme.backgroundColor
                border.color: Kirigami.Theme.textColor
                border.width: 1
                
                x: mediaMouseArea.mouseX - width / 2
                y: mediaMouseArea.mouseY - height / 2
                visible: mediaMouseArea.containsMouse && !mediaViewerDialog.windowTooBig
                
                Kirigami.Icon {
                    anchors.centerIn: parent
                    width: Kirigami.Units.iconSizes.medium
                    height: Kirigami.Units.iconSizes.medium
                    source: mediaViewerDialog.isActualSize ? "zoom-out" : "zoom-in"
                }
            }
        }

        // ── Video player ──
        Item {
            id: videoContainer
            anchors.fill: parent
            visible: mediaViewerDialog.isVideo

            MediaPlayer {
                id: videoPlayer
                source: mediaViewerDialog.isVideo ? mediaViewerDialog.sourceUrl : ""
                videoOutput: videoOutput
                audioOutput: AudioOutput {}

                onSourceChanged: {
                    if (source.toString().length > 0) {
                        play()
                    }
                }
            }

            VideoOutput {
                id: videoOutput
                anchors.fill: parent
            }

            // Play/pause click area
            MouseArea {
                anchors.fill: parent
                onClicked: {
                    if (videoPlayer.playbackState === MediaPlayer.PlayingState) {
                        videoPlayer.pause()
                    } else {
                        videoPlayer.play()
                    }
                }
            }

            // Big play button overlay (paused state)
            Rectangle {
                anchors.centerIn: parent
                width: Kirigami.Units.gridUnit * 4
                height: Kirigami.Units.gridUnit * 4
                radius: width / 2
                color: Qt.rgba(0, 0, 0, 0.55)
                visible: videoPlayer.playbackState !== MediaPlayer.PlayingState

                Kirigami.Icon {
                    anchors.centerIn: parent
                    width: Kirigami.Units.iconSizes.large
                    height: Kirigami.Units.iconSizes.large
                    source: "media-playback-start"
                    color: "white"
                }
            }

            // Video controls bar
            Rectangle {
                anchors.bottom: parent.bottom
                anchors.left: parent.left
                anchors.right: parent.right
                height: videoControlsRow.implicitHeight + Kirigami.Units.largeSpacing * 2
                color: Qt.rgba(0, 0, 0, 0.6)

                RowLayout {
                    id: videoControlsRow
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.largeSpacing

                    // Play/pause button
                    Controls.RoundButton {
                        icon.name: videoPlayer.playbackState === MediaPlayer.PlayingState
                            ? "media-playback-pause" : "media-playback-start"
                        flat: true
                        onClicked: {
                            if (videoPlayer.playbackState === MediaPlayer.PlayingState) {
                                videoPlayer.pause()
                            } else {
                                videoPlayer.play()
                            }
                        }
                        Kirigami.Theme.inherit: false
                        Kirigami.Theme.textColor: "white"
                    }

                    // Position label
                    Controls.Label {
                        text: formatTime(videoPlayer.position) + " / " + formatTime(videoPlayer.duration)
                        color: "white"
                        font: Kirigami.Theme.smallFont
                    }

                    // Seek slider
                    Controls.Slider {
                        id: seekSlider
                        Layout.fillWidth: true
                        from: 0
                        to: videoPlayer.duration > 0 ? videoPlayer.duration : 1
                        value: videoPlayer.position
                        onMoved: videoPlayer.position = value
                    }
                }
            }
        }

        // ── Top toolbar (close + download) ──
        RowLayout {
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing

            Controls.RoundButton {
                icon.name: "download"
                Controls.ToolTip.text: "Save to Downloads"
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                onClicked: {
                    const path = root.messageListModel.save_media(
                        mediaViewerDialog.sourceUrl,
                        mediaViewerDialog.mimeType
                    )
                    if (path.length > 0) {
                        root.showPassiveNotification("Saved to " + path, "long")
                    } else {
                        root.showPassiveNotification("Failed to save media", "short")
                    }
                }
            }

            Controls.RoundButton {
                icon.name: "window-close"
                onClicked: mediaViewerDialog.close()
            }
        }
    }

    function formatTime(ms) {
        if (ms <= 0) return "0:00"
        const totalSec = Math.floor(ms / 1000)
        const min = Math.floor(totalSec / 60)
        const sec = totalSec % 60
        return min + ":" + (sec < 10 ? "0" : "") + sec
    }
}
