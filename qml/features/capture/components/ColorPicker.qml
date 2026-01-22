import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtQuick.Effects
import com.lortunate.minnow

Item {
    id: root

    signal colorCopied

    property string imageSource: ""
    property var screenCapture
    property int mouseX: 0
    property int mouseY: 0
    property int surfaceWidth: 0
    property int surfaceHeight: 0

    property int zoomLevel: 14
    property int magnifierSize: 120
    property int windowMargin: 20

    property color currentColor: "#000000"
    property int colorFormatMode: 0
    property int targetX: mouseX
    property int targetY: mouseY

    readonly property int pickerWidth: 160
    readonly property int pickerHeight: magnifierSize + 100

    x: {
        let desired = targetX + windowMargin;
        if (desired + pickerWidth > (parent ? parent.width : Screen.width)) {
            return targetX - pickerWidth - windowMargin;
        }
        return desired;
    }
    y: {
        let desired = targetY + windowMargin;
        if (desired + pickerHeight > (parent ? parent.height : Screen.height)) {
            return targetY - pickerHeight - windowMargin;
        }
        return desired;
    }

    visible: false

    onMouseXChanged: {
        targetX = mouseX;
        updateColor();
    }
    onMouseYChanged: {
        targetY = mouseY;
        updateColor();
    }
    onVisibleChanged: {
        if (visible)
            updateColor();
    }

    Shortcut {
        enabled: root.visible
        sequence: "Up"
        onActivated: moveCursor(0, -1)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Down"
        onActivated: moveCursor(0, 1)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Left"
        onActivated: moveCursor(-1, 0)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Right"
        onActivated: moveCursor(1, 0)
    }

    function moveCursor(dx, dy) {
        targetX += dx;
        targetY += dy;
        updateColor();
    }

    function updateColor() {
        if (!visible || !screenCapture)
            return;
        let hex = screenCapture.getPixelColor(targetX, targetY, Screen.devicePixelRatio);
        if (hex !== "")
            currentColor = hex;
    }

    function cycleFormat() {
        colorFormatMode = (colorFormatMode + 1) % 2;
    }

    function getFormatLabel() {
        switch (colorFormatMode) {
        case 0:
            return "HEX";
        case 1:
            return "RGB";
        default:
            return "";
        }
    }

    function getFormattedColorString() {
        let c = currentColor;
        switch (colorFormatMode) {
        case 0:
            return c.toString().toUpperCase();
        case 1:
            return Math.round(c.r * 255) + ", " + Math.round(c.g * 255) + ", " + Math.round(c.b * 255);
        default:
            return c.toString();
        }
    }

    function copyColor() {
        if (screenCapture) {
            screenCapture.copyText(getFormattedColorString());
            colorCopied();
        }
    }

    Rectangle {
        id: container
        width: pickerWidth
        height: pickerHeight
        color: AppTheme.surface
        radius: AppTheme.radiusSmall
        border.color: AppTheme.border
        border.width: 1

        layer.enabled: true
        layer.effect: MultiEffect {
            shadowEnabled: true
            shadowBlur: 10
            shadowColor: "#40000000"
            shadowVerticalOffset: 4
        }

        Column {
            anchors.fill: parent
            spacing: 0

            Item {
                width: parent.width
                height: magnifierSize
                clip: true

                Rectangle {
                    anchors.fill: parent
                    color: "black"
                }

                Image {
                    id: zoomImage
                    source: root.imageSource
                    smooth: false
                    fillMode: Image.Stretch
                    width: root.surfaceWidth
                    height: root.surfaceHeight
                    visible: width > 0 && height > 0
                    scale: root.zoomLevel
                    transformOrigin: Item.TopLeft
                    x: (parent.width / 2) - (root.targetX * root.zoomLevel) - (root.zoomLevel / 2)
                    y: (parent.height / 2) - (root.targetY * root.zoomLevel) - (root.zoomLevel / 2)
                }

                Repeater {
                    model: 30
                    Rectangle {
                        width: 1
                        height: parent.height
                        color: "#40808080"
                        x: Math.floor((parent.width - root.zoomLevel) / 2) + (index - 15) * root.zoomLevel
                    }
                }
                Repeater {
                    model: 30
                    Rectangle {
                        height: 1
                        width: parent.width
                        color: "#40808080"
                        y: Math.floor((parent.height - root.zoomLevel) / 2) + (index - 15) * root.zoomLevel
                    }
                }

                Rectangle {
                    x: Math.floor((parent.width - root.zoomLevel) / 2)
                    y: Math.floor((parent.height - root.zoomLevel) / 2)
                    width: root.zoomLevel
                    height: root.zoomLevel
                    color: "transparent"
                    border.color: AppTheme.primary
                    border.width: 2

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: -1
                        color: "transparent"
                        border.color: "#40000000"
                        border.width: 1
                        z: -1
                    }
                }
            }

            Rectangle {
                width: parent.width
                height: 1
                color: AppTheme.border
            }

            Item {
                width: parent.width
                height: parent.height - magnifierSize - 1

                Column {
                    anchors.centerIn: parent
                    spacing: 6
                    width: parent.width - 20

                    Text {
                        text: "X: " + root.targetX + "  Y: " + root.targetY
                        color: AppTheme.subText
                        font.pixelSize: AppTheme.fontSizeSmall
                        font.family: AppTheme.fontFamilyMono
                        anchors.horizontalCenter: parent.horizontalCenter
                    }

                    Row {
                        spacing: 8
                        anchors.horizontalCenter: parent.horizontalCenter
                        height: 20

                        Rectangle {
                            width: 16
                            height: 16
                            color: root.currentColor
                            border.color: AppTheme.border
                            border.width: 1
                            radius: 2
                            anchors.verticalCenter: parent.verticalCenter
                        }

                        Text {
                            text: root.getFormattedColorString()
                            color: AppTheme.text
                            font.bold: true
                            font.pixelSize: AppTheme.fontSizeBody
                            font.family: AppTheme.fontFamilyMono
                            anchors.verticalCenter: parent.verticalCenter
                        }

                        Text {
                            text: root.getFormatLabel()
                            color: AppTheme.subText
                            font.pixelSize: 10
                            font.family: AppTheme.fontFamilyMono
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.verticalCenterOffset: 1
                        }
                    }

                    Text {
                        text: "C: Copy | Shift: Format"
                        color: AppTheme.subText
                        font.pixelSize: 10
                        font.family: AppTheme.fontFamily
                        opacity: 0.7
                        anchors.horizontalCenter: parent.horizontalCenter
                    }
                }
            }
        }
    }
}
