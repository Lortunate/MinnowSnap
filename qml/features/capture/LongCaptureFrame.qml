import QtQuick
import QtQuick.Window
import com.lortunate.minnow

Window {
    id: root

    // Mask Color
    readonly property color maskColor: "#80000000" // Semi-transparent black

    // Padding to ensure visual elements are not captured
    readonly property int padding: 4
    property int selectionHeight: 0
    property int selectionWidth: 0
    property int selectionX: 0
    property int selectionY: 0
    property string warningText: ""

    function flashSuccess() {
        successAnim.restart();
    }

    color: "transparent"

    // Make the window purely visual and pass all input to windows below
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowTransparentForInput | Qt.Tool
    height: Screen.height
    width: Screen.width

    // Full screen to provide dim mask
    x: Screen.virtualX
    y: Screen.virtualY

    // Top Mask
    Rectangle {
        color: maskColor
        height: selectionY - padding
        width: parent.width
        x: 0
        y: 0
    }
    // Bottom Mask
    Rectangle {
        color: maskColor
        height: parent.height - (selectionY + selectionHeight + padding)
        width: parent.width
        x: 0
        y: selectionY + selectionHeight + padding
    }
    // Left Mask
    Rectangle {
        color: maskColor
        height: selectionHeight + (padding * 2)
        width: selectionX - padding
        x: 0
        y: selectionY - padding
    }
    // Right Mask
    Rectangle {
        color: maskColor
        height: selectionHeight + (padding * 2)
        width: parent.width - (selectionX + selectionWidth + padding)
        x: selectionX + selectionWidth + padding
        y: selectionY - padding
    }

    // Border Frame (Visual Feedback)
    // Draw border in the padded area (outside selection)
    Rectangle {
        id: borderRect

        property color pulsedColor: AppTheme.selection

        border.color: warningText !== "" ? AppTheme.danger : (successFlash.opacity > 0 ? AppTheme.success : pulsedColor)
        border.width: warningText !== "" ? 3 : 2
        color: "transparent"
        height: selectionHeight + (padding * 2)
        width: selectionWidth + (padding * 2)
        x: selectionX - padding
        y: selectionY - padding

        SequentialAnimation on pulsedColor {
            loops: Animation.Infinite
            running: warningText === "" && successFlash.opacity === 0

            ColorAnimation {
                duration: 800
                easing.type: Easing.InOutQuad
                from: AppTheme.selection
                to: Qt.lighter(AppTheme.selection, 1.4)
            }
            ColorAnimation {
                duration: 800
                easing.type: Easing.InOutQuad
                to: AppTheme.selection
            }
        }

        // Success Flash
        Rectangle {
            id: successFlash

            anchors.fill: parent
            border.color: AppTheme.success
            border.width: 3
            color: "transparent"
            opacity: 0

            NumberAnimation on opacity {
                id: successAnim

                duration: 600
                easing.type: Easing.OutQuad
                from: 1.0
                running: false
                to: 0.0
            }
        }

        // Warning Bubble
        Rectangle {
            anchors.bottom: parent.top
            anchors.bottomMargin: 12
            anchors.horizontalCenter: parent.horizontalCenter
            color: AppTheme.danger
            height: warnTxt.contentHeight + 12
            radius: 6
            visible: warningText !== ""
            width: warnTxt.contentWidth + 24

            Text {
                id: warnTxt

                anchors.centerIn: parent
                color: "white"
                font.bold: true
                font.pixelSize: AppTheme.fontSizeBody
                text: warningText
            }
        }
    }
}
