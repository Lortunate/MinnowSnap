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

    property string imageSource: ""
    property int shadowMargin: 20
    property var screenCapture: null
    property bool autoOcr: false

    visible: true
    width: 300
    height: 200
    color: "transparent"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.ToolTip

    Component.onCompleted: {
        if (pinWindow.screenCapture) {
            pinWindow.screenCapture.incrementPinCount();
        }
        pinWindow.raise();
        if (autoOcr) {
            ocrOverlay.recognize();
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
            pinWindow.close();
        }
    }

    QtObject {
        id: menuActions

        function copy() {
            var focusItem = pinWindow.activeFocusItem;
            if (focusItem && typeof focusItem.copy === "function" && focusItem.selectedText && focusItem.selectedText.length > 0) {
                focusItem.copy();
                focusItem.deselect();
                return;
            }

            if (ocrOverlay.copySelection()) {
                return;
            }

            if (pinWindow.screenCapture) {
                pinWindow.screenCapture.copyImage(pinWindow.imageSource, 0, 0, 0, 0);
            }
        }

        function save() {
            if (pinWindow.screenCapture) {
                pinWindow.screenCapture.saveImage(pinWindow.imageSource, 0, 0, 0, 0);
            }
        }

        function close() {
            pinWindow.close();
        }

        function closeAll() {
            if (pinWindow.screenCapture) {
                pinWindow.screenCapture.emitCloseAllPins();
            }
        }

        function triggerOcr() {
            ocrOverlay.recognize();
        }
    }

    Shortcut {
        sequence: StandardKey.Copy
        onActivated: menuActions.copy()
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
                source: pinWindow.imageSource
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
        shadowColor: "#000000"
        shadowOpacity: 0.5
        shadowBlur: 1.0
        shadowVerticalOffset: 4
    }

    MouseArea {
        id: mouseArea
        anchors.fill: contentItem
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
        sourcePath: pinWindow.imageSource
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
            enabled: pinWindow.screenCapture !== null
            onTriggered: menuActions.copy()
        }

        Platform.MenuItem {
            text: qsTr("Save")
            enabled: pinWindow.screenCapture !== null
            onTriggered: menuActions.save()
        }

        Platform.MenuItem {
            text: qsTr("OCR")
            onTriggered: menuActions.triggerOcr()
        }

        Platform.MenuSeparator {}

        Platform.MenuItem {
            text: qsTr("Close")
            onTriggered: menuActions.close()
        }

        Platform.MenuItem {
            text: qsTr("Close All")
            enabled: pinWindow.screenCapture !== null
            visible: pinWindow.screenCapture && pinWindow.screenCapture.pinCount > 1
            onTriggered: menuActions.closeAll()
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
                    enabled: pinWindow.screenCapture !== null,
                    action: menuActions.copy
                },
                {
                    text: qsTr("Save"),
                    enabled: pinWindow.screenCapture !== null,
                    action: menuActions.save
                },
                {
                    text: qsTr("OCR"),
                    enabled: true,
                    action: menuActions.triggerOcr
                },
                {
                    isDivider: true
                },
                {
                    text: qsTr("Close"),
                    enabled: true,
                    action: menuActions.close
                }
            ];

            if (pinWindow.screenCapture && pinWindow.screenCapture.pinCount > 1) {
                items.push({
                    text: qsTr("Close All"),
                    enabled: pinWindow.screenCapture !== null,
                    action: menuActions.closeAll
                });
            }

            return items;
        }
    }
}
