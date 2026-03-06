import QtQuick
import QtQuick.Window
import com.lortunate.minnow

Window {
    id: root

    readonly property color maskColor: AppTheme.overlayMask

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

    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowTransparentForInput | Qt.Tool
    height: Screen.height
    width: Screen.width

    x: Screen.virtualX
    y: Screen.virtualY

    Rectangle {
        color: maskColor
        height: selectionY - padding
        width: parent.width
        x: 0
        y: 0
    }
    Rectangle {
        color: maskColor
        height: parent.height - (selectionY + selectionHeight + padding)
        width: parent.width
        x: 0
        y: selectionY + selectionHeight + padding
    }
    Rectangle {
        color: maskColor
        height: selectionHeight + (padding * 2)
        width: selectionX - padding
        x: 0
        y: selectionY - padding
    }
    Rectangle {
        color: maskColor
        height: selectionHeight + (padding * 2)
        width: parent.width - (selectionX + selectionWidth + padding)
        x: selectionX + selectionWidth + padding
        y: selectionY - padding
    }

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
