import QtQuick
import QtQuick.Controls
import QtQuick.Effects
import QtQuick.Window
import Qt.labs.platform as Platform
import com.lortunate.minnow
import "../../components"
import "components"

Window {
    id: pinWindow

    property alias imageSource: controller.imagePath
    property int shadowMargin: 20
    property var screenCapture: null

    visible: true
    width: 300
    height: 200
    color: "transparent"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.ToolTip

    PinController {
        id: controller
        onCloseRequested: pinWindow.close()
        onCloseAllRequested: {
            if (pinWindow.screenCapture) {
                pinWindow.screenCapture.emitCloseAllPins();
            }
        }
        onOcrRequested: ocrOverlay.recognize()
    }

    Component.onCompleted: {
        if (pinWindow.screenCapture) {
            pinWindow.screenCapture.incrementPinCount();
        }
        pinWindow.raise();
        if (controller.autoOcr) {
            ocrOverlay.triggerOcr();
        }
    }

    Component.onDestruction: {
        if (pinWindow.screenCapture) {
            pinWindow.screenCapture.decrementPinCount();
        }
    }

    onClosing: pinWindow.destroy()

    Connections {
        target: pinWindow.screenCapture
        ignoreUnknownSignals: true
        function onCloseAllPins() {
            controller.close();
        }
    }

    Shortcut {
        sequences: [StandardKey.Copy]
        onActivated: {
            if (ocrOverlay.copySelection()) return;
            controller.copyImage();
        }
    }

    Shortcut {
        sequences: [StandardKey.Cancel]
        onActivated: {
            if (ocrOverlay.hasSelection || ocrOverlay.activeTextBlock) {
                ocrOverlay.clearSelection();
            } else {
                controller.close();
            }
        }
    }

    Item {
        id: contentItem
        anchors.fill: parent
        anchors.margins: pinWindow.shadowMargin
        visible: false

        Rectangle {
            anchors.fill: parent
            color: "transparent"
            radius: AppTheme.radiusLarge

            Image {
                id: img
                anchors.fill: parent
                source: imageSource
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                mipmap: true
                smooth: true
            }
        }
    }

    MultiEffect {
        id: shadowEffect
        anchors.fill: contentItem
        source: contentItem
        shadowEnabled: true
        shadowColor: AppTheme.shadowColor
        shadowOpacity: 0.5
        shadowBlur: 1.0
        shadowVerticalOffset: 4
    }

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton

        onPressed: mouse => {
            if (mouse.button === Qt.LeftButton) {
                customMenu.visible = false;
                pinWindow.startSystemMove();
            }
        }

        onClicked: mouse => {
            if (mouse.button === Qt.RightButton) {
                if (Qt.platform.os === "osx") {
                    nativeMenu.open();
                } else {
                    const point = mouseArea.mapToGlobal(mouse.x, mouse.y);
                    customMenu.popup(point.x, point.y);
                }
            }
        }
    }

    MouseArea {
        id: wheelHandler
        anchors.fill: parent
        acceptedButtons: Qt.NoButton
        z: 100

        onWheel: wheel => {
            customMenu.visible = false;
            const factor = wheel.angleDelta.y > 0 ? 1.1 : 0.9;
            const newW = pinWindow.width * factor;
            const newH = pinWindow.height * factor;
            const maxTextureSize = 16384 - 50;
            const dpr = Screen.devicePixelRatio || 1.0;
            const maxSize = maxTextureSize / dpr;

            if (newW > 100 && newH > 100 && newW < maxSize && newH < maxSize) {
                const dw = newW - pinWindow.width;
                const dh = newH - pinWindow.height;
                pinWindow.x -= dw / 2;
                pinWindow.y -= dh / 2;
                pinWindow.width = newW;
                pinWindow.height = newH;
            }
        }
    }

    OcrOverlay {
        id: ocrOverlay
        anchors.fill: contentItem
        z: 10
        targetImage: img
        sourcePath: imageSource
        onRequestMenu: (x, y) => {
            if (Qt.platform.os === "osx") {
                nativeMenu.open();
            } else {
                const point = ocrOverlay.mapToGlobal(x, y);
                customMenu.popup(point.x, point.y);
            }
        }
    }

    Platform.Menu {
        id: nativeMenu

        Platform.MenuItem {
            text: qsTr("Copy")
            onTriggered: controller.copyImage()
        }

        Platform.MenuItem {
            text: qsTr("Save")
            onTriggered: controller.saveImage()
        }

        Platform.MenuItem {
            text: qsTr("OCR")
            onTriggered: controller.triggerOcr()
        }

        Platform.MenuSeparator {}

        Platform.MenuItem {
            text: qsTr("Close")
            onTriggered: controller.close()
        }

        Platform.MenuItem {
            text: qsTr("Close All")
            visible: pinWindow.screenCapture && pinWindow.screenCapture.pinCount > 1
            onTriggered: controller.closeAll()
        }
    }

    ContextMenu {
        id: customMenu
        transientParent: pinWindow
        flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.NoDropShadowWindowHint

        onActiveChanged: {
            if (!active) {
                visible = false;
            }
        }

        menuItems: {
            const items = [
                {
                    text: qsTr("Copy"),
                    action: controller.copyImage
                },
                {
                    text: qsTr("Save"),
                    action: controller.saveImage
                },
                {
                    text: qsTr("OCR"),
                    action: controller.triggerOcr
                },
                {
                    isDivider: true
                },
                {
                    text: qsTr("Close"),
                    action: controller.close
                }
            ];

            if (pinWindow.screenCapture && pinWindow.screenCapture.pinCount > 1) {
                items.push({
                    text: qsTr("Close All"),
                    action: controller.closeAll
                });
            }

            return items;
        }
    }
}
