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

    property int zoomLevel: 16
    property int magnifierSize: 140
    property int windowMargin: 20
    property int samplingRadius: 3

    property color currentColor: "#000000"
    property int colorFormatMode: 0
    property int targetX: mouseX
    property int targetY: mouseY
    property bool isPickingActive: false

    readonly property int pickerWidth: 180
    readonly property int pickerHeight: magnifierSize + 110
    readonly property real devicePixelRatio: Screen.devicePixelRatio

    x: {
        let desired = targetX + windowMargin;
        if (desired + pickerWidth > (parent ? parent.width : Screen.width)) {
            return Math.max(windowMargin, targetX - pickerWidth - windowMargin);
        }
        return desired;
    }
    y: {
        let desired = targetY + windowMargin;
        if (desired + pickerHeight > (parent ? parent.height : Screen.height)) {
            return Math.max(windowMargin, targetY - pickerHeight - windowMargin);
        }
        return desired;
    }

    visible: false

    onMouseXChanged: {
        if (!isPickingActive) {
            targetX = Math.max(0, Math.min(mouseX, surfaceWidth - 1));
            updateColorDebounced();
        }
    }
    onMouseYChanged: {
        if (!isPickingActive) {
            targetY = Math.max(0, Math.min(mouseY, surfaceHeight - 1));
            updateColorDebounced();
        }
    }
    onVisibleChanged: {
        if (visible) {
            isPickingActive = true;
            updateColor();
            isPickingActive = false;
        }
    }

    Shortcut {
        enabled: root.visible
        sequence: "Up"
        onActivated: adjustCursor(0, -1)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Down"
        onActivated: adjustCursor(0, 1)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Left"
        onActivated: adjustCursor(-1, 0)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Right"
        onActivated: adjustCursor(1, 0)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Shift+Up"
        onActivated: adjustCursor(0, -5)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Shift+Down"
        onActivated: adjustCursor(0, 5)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Shift+Left"
        onActivated: adjustCursor(-5, 0)
    }
    Shortcut {
        enabled: root.visible
        sequence: "Shift+Right"
        onActivated: adjustCursor(5, 0)
    }

    function adjustCursor(dx, dy) {
        let newX = Math.max(0, Math.min(targetX + dx, surfaceWidth - 1));
        let newY = Math.max(0, Math.min(targetY + dy, surfaceHeight - 1));

        if (newX !== targetX || newY !== targetY) {
            targetX = newX;
            targetY = newY;
            updateColor();
            if (screenCapture) {
                screenCapture.setCursorPosition(targetX, targetY, devicePixelRatio);
            }
        }
    }

    Timer {
        id: updateDebounceTimer
        interval: 16
        repeat: false
        onTriggered: updateColor()
    }

    function updateColorDebounced() {
        updateDebounceTimer.restart();
    }

    function updateColor() {
        if (!visible || !screenCapture || surfaceWidth <= 0 || surfaceHeight <= 0)
            return;

        let clampedX = Math.max(0, Math.min(targetX, surfaceWidth - 1));
        let clampedY = Math.max(0, Math.min(targetY, surfaceHeight - 1));

        let hex = screenCapture.getPixelColor(clampedX, clampedY, devicePixelRatio);

        if (hex && hex.length > 0 && hex !== "#") {
            currentColor = hex;
        }
    }

    function cycleFormat() {
        colorFormatMode = (colorFormatMode + 1) % 3;
    }

    function getFormatLabel() {
        switch (colorFormatMode) {
        case 0:
            return "HEX";
        case 1:
            return "RGB";
        case 2:
            return "HSL";
        default:
            return "";
        }
    }

    function rgbToHsl(r, g, b) {
        r = r / 255;
        g = g / 255;
        b = b / 255;

        let max = Math.max(r, g, b);
        let min = Math.min(r, g, b);
        let l = (max + min) / 2;
        let h, s;

        if (max === min) {
            h = s = 0;
        } else {
            let d = max - min;
            s = l > 0.5 ? d / (2 - max - min) : d / (max + min);

            switch (max) {
            case r:
                h = (g - b) / d + (g < b ? 6 : 0);
                break;
            case g:
                h = (b - r) / d + 2;
                break;
            case b:
                h = (r - g) / d + 4;
                break;
            }
            h /= 6;
        }

        return {
            h: Math.round(h * 360),
            s: Math.round(s * 100),
            l: Math.round(l * 100)
        };
    }

    function getFormattedColorString() {
        let c = currentColor;
        switch (colorFormatMode) {
        case 0:
            return c.toString().toUpperCase();
        case 1:
            {
                let r = Math.round(c.r * 255);
                let g = Math.round(c.g * 255);
                let b = Math.round(c.b * 255);
                return r + ", " + g + ", " + b;
            }
        case 2:
            {
                let r = Math.round(c.r * 255);
                let g = Math.round(c.g * 255);
                let b = Math.round(c.b * 255);
                let hsl = rgbToHsl(r, g, b);
                return hsl.h + "°, " + hsl.s + "%, " + hsl.l + "%";
            }
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
            shadowBlur: 12
            shadowColor: "#50000000"
            shadowVerticalOffset: 4
            shadowHorizontalOffset: 0
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
                    color: "#1a1a1a"
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
                    model: 35
                    Rectangle {
                        width: 1
                        height: parent.height
                        color: "#30606060"
                        x: Math.floor((parent.width - root.zoomLevel) / 2) + (index - 17) * root.zoomLevel
                    }
                }
                Repeater {
                    model: 35
                    Rectangle {
                        height: 1
                        width: parent.width
                        color: "#30606060"
                        y: Math.floor((parent.height - root.zoomLevel) / 2) + (index - 17) * root.zoomLevel
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
                        border.color: "#50000000"
                        border.width: 1
                        z: -1
                    }
                }

                Rectangle {
                    x: Math.floor((parent.width - root.zoomLevel) / 2) - 1
                    y: -1
                    width: root.zoomLevel + 2
                    height: 1
                    color: AppTheme.primary
                    opacity: 0.4
                }
                Rectangle {
                    x: -1
                    y: Math.floor((parent.height - root.zoomLevel) / 2)
                    width: 1
                    height: root.zoomLevel
                    color: AppTheme.primary
                    opacity: 0.4
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
                    spacing: 8
                    width: parent.width - 24

                    Text {
                        text: "X: " + root.targetX + "  Y: " + root.targetY
                        color: AppTheme.subText
                        font.pixelSize: AppTheme.fontSizeSmall
                        font.family: AppTheme.fontFamilyMono
                        anchors.horizontalCenter: parent.horizontalCenter
                    }

                    Row {
                        spacing: 10
                        anchors.horizontalCenter: parent.horizontalCenter
                        height: 24

                        Rectangle {
                            width: 20
                            height: 20
                            color: root.currentColor
                            border.color: AppTheme.border
                            border.width: 1
                            radius: 3
                            anchors.verticalCenter: parent.verticalCenter
                        }

                        Text {
                            text: root.getFormattedColorString()
                            color: AppTheme.text
                            font.bold: true
                            font.pixelSize: AppTheme.fontSizeBody
                            font.family: AppTheme.fontFamilyMono
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideRight
                            maximumLineCount: 1
                        }

                        Text {
                            text: root.getFormatLabel()
                            color: AppTheme.subText
                            font.pixelSize: 9
                            font.family: AppTheme.fontFamilyMono
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }

                    Text {
                        text: "C: Copy | Shift: Format | Arrow: Move"
                        color: AppTheme.subText
                        font.pixelSize: 9
                        font.family: AppTheme.fontFamily
                        opacity: 0.65
                        anchors.horizontalCenter: parent.horizontalCenter
                        wrapMode: Text.Wrap
                    }
                }
            }
        }
    }
}
