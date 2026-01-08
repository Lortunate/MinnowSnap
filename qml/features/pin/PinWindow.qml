import QtQuick
import QtQuick.Window
import QtQuick.Effects
import QtQuick.Controls
import QtCore
import com.lortunate.minnow
import "../../components"

Window {
    id: pinWindow

    property string imageSource: ""
    property int shadowMargin: 20
    property var screenCapture: null

    color: "transparent"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.ToolTip
    width: 300
    height: 200
    visible: true

    Component.onCompleted: {
        pinWindow.raise();
    }

    onClosing: {
        pinWindow.destroy();
    }

    Connections {
        target: pinWindow.screenCapture
        ignoreUnknownSignals: true
        function onCloseAllPins() {
            pinWindow.close();
        }
    }

    Item {
        id: contentItem

        anchors.fill: parent
        anchors.margins: pinWindow.shadowMargin

        Rectangle {
            id: bgRect
            anchors.fill: parent
            color: "transparent"
            radius: AppTheme.radiusLarge

            Image {
                id: img

                anchors.fill: parent
                asynchronous: true
                fillMode: Image.PreserveAspectFit
                mipmap: true
                smooth: true
                source: pinWindow.imageSource
            }
        }
    }

    MultiEffect {
        id: shadowEffect

        anchors.fill: contentItem
        shadowBlur: 1.0
        shadowColor: "#000000"
        shadowEnabled: true
        shadowOpacity: 0.5
        shadowVerticalOffset: 4
        source: contentItem
    }

    MouseArea {
        id: mouseArea
        anchors.fill: contentItem
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        hoverEnabled: true

        onPressed: mouse => {
            if (mouse.button === Qt.LeftButton) {
                pinWindow.startSystemMove();
            }
        }

        onClicked: mouse => {
            if (mouse.button === Qt.RightButton) {
                var point = mouseArea.mapToGlobal(mouse.x, mouse.y);
                contextMenu.popup(point.x, point.y);
            }
        }

        onWheel: wheel => {
            var factor = wheel.angleDelta.y > 0 ? 1.1 : 0.9;
            var newW = pinWindow.width * factor;
            var newH = pinWindow.height * factor;

            // Limit size
            var maxTextureSize = 16384 - 50;
            var dpr = Screen.devicePixelRatio || 1.0;
            var maxSize = maxTextureSize / dpr;

            if (newW > 100 && newH > 100 && newW < maxSize && newH < maxSize) {
                var dw = newW - pinWindow.width;
                var dh = newH - pinWindow.height;
                pinWindow.x -= dw / 2;
                pinWindow.y -= dh / 2;
                pinWindow.width = newW;
                pinWindow.height = newH;
            }
        }
    }

    ContextMenu {
        id: contextMenu

        menuItems: [
            {
                text: qsTr("Copy"),
                enabled: pinWindow.screenCapture !== null,
                action: function () {
                    if (pinWindow.screenCapture) {
                        pinWindow.screenCapture.copyImage(pinWindow.imageSource, 0, 0, 0, 0);
                    }
                }
            },
            {
                text: qsTr("Save"),
                enabled: pinWindow.screenCapture !== null,
                action: function () {
                    if (pinWindow.screenCapture) {
                        pinWindow.screenCapture.saveImage(pinWindow.imageSource, 0, 0, 0, 0);
                    }
                }
            },
            {
                isDivider: true
            },
            {
                text: qsTr("Close"),
                enabled: true,
                action: function () {
                    pinWindow.close();
                }
            },
            {
                text: qsTr("Close All"),
                enabled: pinWindow.screenCapture !== null,
                action: function () {
                    if (pinWindow.screenCapture) {
                        pinWindow.screenCapture.emitCloseAllPins();
                    }
                }
            }
        ]
    }
}
