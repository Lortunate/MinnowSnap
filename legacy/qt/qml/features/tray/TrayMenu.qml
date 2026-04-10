import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtQuick.Layouts
import com.lortunate.minnow
import "../../components"

Window {
    id: root

    signal preferencesRequested()
    signal screenCaptureRequested()
    signal quickCaptureRequested()
    signal quitRequested()
    property alias controller: trayMenuController

    flags: Qt.Popup | Qt.FramelessWindowHint | Qt.NoDropShadowWindowHint
    visible: trayMenuController.popupVisible
    x: trayMenuController.popupX
    y: trayMenuController.popupY
    width: layout.implicitWidth + 16
    height: layout.height + 16
    color: "transparent"

    function popup(geometry) {
        trayMenuController.popup(geometryToJson(geometry), screensToJson(), Qt.platform.os, width, height);
        if (trayMenuController.popupVisible) {
            show();
            raise();
            requestActivate();
        }
    }

    function geometryToJson(geometry) {
        if (!geometry) {
            return "null";
        }
        return JSON.stringify({
            "x": geometry.x,
            "y": geometry.y,
            "width": geometry.width,
            "height": geometry.height
        });
    }

    function screensToJson() {
        var serialized = [];
        var screens = Qt.application.screens;
        for (var i = 0; i < screens.length; i++) {
            var s = screens[i];
            serialized.push({
                "virtualX": s.virtualX,
                "virtualY": s.virtualY,
                "width": s.width,
                "height": s.height,
                "devicePixelRatio": s.devicePixelRatio
            });
        }
        return JSON.stringify(serialized);
    }

    TrayMenuController {
        id: trayMenuController
    }

    onActiveChanged: {
        trayMenuController.syncWindowActive(active);
    }

    Rectangle {
        id: bg
        anchors.fill: parent
        color: AppTheme.surface
        radius: AppTheme.radiusLarge
        border.color: AppTheme.border
        border.width: 1

        ColumnLayout {
            id: layout
            anchors.centerIn: parent
            width: parent.width - 16
            spacing: 2

            TrayMenuItem {
                text: qsTr("Preferences")
                onClicked: {
                    root.preferencesRequested();
                    trayMenuController.hideMenu();
                }
            }

            Divider {
                Layout.topMargin: 1
                Layout.bottomMargin: 1
            }

            TrayMenuItem {
                text: qsTr("Capture")
                onClicked: {
                    root.screenCaptureRequested();
                    trayMenuController.hideMenu();
                }
            }

            TrayMenuItem {
                text: qsTr("Quick Capture")
                onClicked: {
                    root.quickCaptureRequested();
                    trayMenuController.hideMenu();
                }
            }

            Divider {
                Layout.topMargin: 1
                Layout.bottomMargin: 1
            }

            TrayMenuItem {
                text: qsTr("Exit")
                onClicked: {
                    root.quitRequested();
                    trayMenuController.hideMenu();
                }
            }
        }
    }

    component TrayMenuItem: AbstractButton {
        id: control

        Layout.fillWidth: true
        implicitWidth: contentItem.implicitWidth + 32
        implicitHeight: AppTheme.buttonHeight + 4

        background: Rectangle {
            implicitHeight: control.implicitHeight
            color: control.hovered ? AppTheme.itemHover : "transparent"
            radius: AppTheme.radiusSmall
        }

        contentItem: Text {
            text: control.text
            color: AppTheme.text
            font.family: AppTheme.fontFamily
            font.pixelSize: AppTheme.fontSizeBody
            verticalAlignment: Text.AlignVCenter
            leftPadding: 16
            rightPadding: 16
            elide: Text.ElideRight
        }
    }
}
