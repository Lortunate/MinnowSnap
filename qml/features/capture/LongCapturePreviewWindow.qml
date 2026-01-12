import QtQuick
import QtQuick.Window
import com.lortunate.minnow

Window {
    id: root

    // Calculate expected height to fit the image aspect ratio
    // currentHeight is physical px. selectionWidth is logical px.
    readonly property real contentHeight: {
        if (selectionWidth <= 0)
            return 150;
        let scale = Screen.devicePixelRatio || 2; // Default to 2 if unsure, safe bet
        let imgPhysicalW = selectionWidth * scale;
        if (imgPhysicalW <= 0)
            return 150;

        let viewW = width;
        let ratio = viewW / imgPhysicalW;
        let h = currentHeight * ratio;
        return Math.max(100, h);
    }
    property int currentHeight: 0
    property int selectionHeight: 0
    property int selectionWidth: 0
    property int selectionX: 0
    property int selectionY: 0
    property bool showFull: false // Toggle between full view and latest view

    property int updateCounter: 0

    function refresh(h) {
        updateCounter += 1;
        if (h)
            currentHeight = h;
    }

    color: "transparent"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool
    height: Math.min(contentHeight, 600)

    width: 300

    // Position logic: Keep it to the side.
    x: {
        let rightSpace = Screen.width - (selectionX + selectionWidth);
        if (rightSpace > width + 20)
            return selectionX + selectionWidth + 20;
        return selectionX - width - 20;
    }
    y: Math.max(Math.min(selectionY, Screen.height - height - 20), 20)

    // Shadow for depth
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

        // Interaction Area (Drag & Click)
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
            // Default to cropping (showing latest content) for better feedback during scroll
            fillMode: root.showFull ? Image.PreserveAspectFit : Image.PreserveAspectCrop
            mipmap: true
            smooth: true
            source: "image://minnow/scroll?t=" + root.updateCounter
            sourceSize.width: parent.width * Screen.devicePixelRatio
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

        // Minimal Floating Badge
        Rectangle {
            anchors.bottom: parent.bottom
            anchors.margins: 8
            anchors.right: parent.right
            color: AppTheme.primary
            height: 24
            opacity: 0.9

            // Add shadow to badge too
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
