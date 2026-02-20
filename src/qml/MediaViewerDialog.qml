import QtQuick
import QtQuick.Controls as Controls
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
    property bool isActualSize: false
    property bool windowTooBig: viewerContent.width >= imageActual.implicitWidth && viewerContent.height >= imageActual.implicitHeight
    
    onClosed: {
        sourceUrl = ""
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
        
        Flickable {
            id: imageFlickable
            anchors.fill: parent
            contentWidth: mediaViewerDialog.isActualSize ? imageActual.implicitWidth : width
            contentHeight: mediaViewerDialog.isActualSize ? imageActual.implicitHeight : height
            interactive: mediaViewerDialog.isActualSize && !mediaViewerDialog.windowTooBig
            
            Image {
                id: imageActual
                source: mediaViewerDialog.sourceUrl
                fillMode: mediaViewerDialog.isActualSize ? Image.Pad : Image.PreserveAspectFit
                width: mediaViewerDialog.isActualSize ? implicitWidth : imageFlickable.width
                height: mediaViewerDialog.isActualSize ? implicitHeight : imageFlickable.height
                
                x: Math.max(0, (imageFlickable.width - width) / 2)
                y: Math.max(0, (imageFlickable.height - height) / 2)
            }
        }
        
        MouseArea {
            id: mediaMouseArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: mediaViewerDialog.windowTooBig ? Qt.ArrowCursor : Qt.BlankCursor
            
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
        
        Controls.RoundButton {
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: Kirigami.Units.largeSpacing
            icon.name: "window-close"
            onClicked: mediaViewerDialog.close()
        }
    }
}
