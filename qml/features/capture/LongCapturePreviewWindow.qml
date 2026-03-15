import QtQuick
import QtQuick.Window
import com.lortunate.minnow

Window {
    id: root

    readonly property real contentHeight: {
        if (selectionRect.width <= 0)
            return 150
        let scale = viewportScale > 0 ? viewportScale : 1
        let imgPhysicalW = selectionRect.width * scale
        if (imgPhysicalW <= 0)
            return 150

        let ratio = width / imgPhysicalW
        let h = currentHeight * ratio
        return Math.max(100, h)
    }
    property int currentHeight: 0
    property rect selectionRect: Qt.rect(0, 0, 0, 0)
    property rect viewportRect: Qt.rect(0, 0, 0, 0)
    property real viewportScale: 1.0
    property int sourceRevision: 0
    property bool showFull: false
    readonly property string scrollSourceBase: "image://minnow/scroll"
    readonly property string scrollSource: scrollSourceBase + "?rev=" + sourceRevision

    function refresh(h) {
        if (h)
            currentHeight = h
        if (!root.visible)
            return
        sourceRevision += 1
    }

    color: "transparent"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool
    height: Math.min(contentHeight, 600)

    width: 300

    x: {
        let rightSpace = viewportRect.width - (selectionRect.x + selectionRect.width)
        let preferred = rightSpace > width + 20 ? (selectionRect.x + selectionRect.width + 20) : (selectionRect.x - width - 20)
        let minX = 20
        let maxX = Math.max(minX, viewportRect.width - width - 20)
        let localX = Math.max(minX, Math.min(maxX, preferred))
        return viewportRect.x + localX
    }
    y: viewportRect.y + Math.max(Math.min(selectionRect.y, viewportRect.height - height - 20), 20)

    Rectangle {
        color: AppTheme.shadowMedium
        height: parent.height
        radius: AppTheme.radiusLarge
        width: parent.width
        x: 4
        y: 4
    }
    Rectangle {
        id: mainRect

        anchors.fill: parent
        clip: true
        color: AppTheme.surface
        radius: AppTheme.radiusLarge

        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onPressed: root.startSystemMove()

            onClicked: {
                root.showFull = !root.showFull;
            }
        }
        Image {
            id: previewImg

            anchors.fill: parent
            cache: false
            fillMode: root.showFull ? Image.PreserveAspectFit : Image.PreserveAspectCrop
            mipmap: false
            smooth: true
            source: root.visible ? root.scrollSource : ""
            sourceSize.width: parent.width * (viewportScale > 0 ? viewportScale : 1)
            verticalAlignment: Image.AlignBottom

            Item {
                anchors.centerIn: parent
                height: 24
                visible: root.currentHeight === 0
                width: 24

                RotationAnimator on rotation {
                    duration: 800
                    from: 0
                    loops: Animation.Infinite
                    to: 360
                }

                Canvas {
                    anchors.fill: parent
                    antialiasing: true
                    renderTarget: Canvas.Image

                    onPaint: {
                        var ctx = getContext("2d");
                        ctx.reset();
                        ctx.beginPath();
                        ctx.arc(12, 12, 9, 0, Math.PI * 1.5);
                        ctx.lineWidth = 2;
                        ctx.lineCap = "round";
                        ctx.strokeStyle = AppTheme.primary;
                        ctx.stroke();
                    }
                }
            }
            Text {
                anchors.centerIn: parent
                anchors.verticalCenterOffset: 24
                color: AppTheme.subText
                font.bold: true
                font.pixelSize: 12
                text: qsTr("Scroll to Capture")
                visible: root.currentHeight === 0
            }
        }

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.margins: 8
            anchors.right: parent.right
            color: AppTheme.primary
            height: 24
            opacity: 0.9

            layer.enabled: true
            radius: 12
            visible: root.currentHeight > 0
            width: badgeTxt.contentWidth + AppTheme.spacingMedium

            Text {
                id: badgeTxt

                anchors.centerIn: parent
                color: "white"
                font.bold: true
                font.family: AppTheme.fontFamily
                font.pixelSize: AppTheme.fontSizeSmall
                text: root.currentHeight + " px"
            }
        }
    }
}
