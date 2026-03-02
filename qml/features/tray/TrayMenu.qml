import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtQuick.Layouts
import com.lortunate.minnow
import "../../components"

Window {
    id: root

    property Action preferencesAction
    property Action quickCaptureAction
    property Action screenCaptureAction
    property Action quitAction

    flags: Qt.Popup | Qt.FramelessWindowHint | Qt.NoDropShadowWindowHint
    visible: false
    width: layout.implicitWidth + 16
    height: layout.height + 16
    color: "transparent"

    function popup(geometry) {
        if (!geometry) {
            if (Qt.application.screens.length > 0) {
                var s = Qt.application.screens[0];
                x = s.virtualX + (s.width - width) / 2;
                y = s.virtualY + (s.height - height) / 2;
                show();
            }
            return;
        }

        var rect = _getLogicalGeometry(geometry);
        var currentScreen = _findScreen(rect);
        
        if (!currentScreen) return;

        var targetPos = _calculatePosition(rect, currentScreen);
        
        x = targetPos.x;
        y = targetPos.y;

        show();
        raise();
        requestActivate();
    }

    function _getLogicalGeometry(geometry) {
        var rect = {
            x: geometry.x,
            y: geometry.y,
            width: geometry.width,
            height: geometry.height
        };

        if (Qt.platform.os !== "windows") return rect;

        var screens = Qt.application.screens;
        var isLogical = false;

        for (var i = 0; i < screens.length; i++) {
            var s = screens[i];
            if (rect.x >= s.virtualX && rect.x < s.virtualX + s.width &&
                rect.y >= s.virtualY && rect.y < s.virtualY + s.height) {
                isLogical = true;
                break;
            }
        }

        if (!isLogical) {
            for (var j = 0; j < screens.length; j++) {
                var sc = screens[j];
                var dpr = sc.devicePixelRatio;
                var px = rect.x / dpr;
                var py = rect.y / dpr;
                if (px >= sc.virtualX && px < sc.virtualX + sc.width &&
                    py >= sc.virtualY && py < sc.virtualY + sc.height) {
                    rect.x = px;
                    rect.y = py;
                    rect.width /= dpr;
                    rect.height /= dpr;
                    break;
                }
            }
        }
        return rect;
    }

    function _findScreen(rect) {
        var cx = rect.x + (rect.width / 2);
        var cy = rect.y + (rect.height / 2);
        var screens = Qt.application.screens;
        
        for (var i = 0; i < screens.length; i++) {
            var s = screens[i];
            if (cx >= s.virtualX && cx < s.virtualX + s.width &&
                cy >= s.virtualY && cy < s.virtualY + s.height) {
                return s;
            }
        }
        return screens.length > 0 ? screens[0] : null;
    }

    function _calculatePosition(rect, screen) {
        var cy = rect.y + (rect.height / 2);
        
        var targetX = rect.x;
        var targetY = rect.y + rect.height + 5;

        if (cy > screen.virtualY + (screen.height / 2)) {
            targetY = rect.y - height - 5;
        }

        var padding = 6;
        
        if (targetX + width > screen.virtualX + screen.width - padding) {
            targetX = screen.virtualX + screen.width - width - padding;
        }
        if (targetX < screen.virtualX + padding) {
            targetX = screen.virtualX + padding;
        }

        if (targetY + height > screen.virtualY + screen.height - padding) {
            targetY = screen.virtualY + screen.height - height - padding;
        }
        if (targetY < screen.virtualY + padding) {
            targetY = screen.virtualY + padding;
        }

        return { x: targetX, y: targetY };
    }

    onActiveChanged: {
        if (!active)
            visible = false;
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
                action: root.preferencesAction
                onClicked: root.visible = false
            }

            Divider {
                Layout.topMargin: 1
                Layout.bottomMargin: 1
            }

            TrayMenuItem {
                action: root.screenCaptureAction
                onClicked: root.visible = false
            }

            TrayMenuItem {
                action: root.quickCaptureAction
                onClicked: root.visible = false
            }

            Divider {
                Layout.topMargin: 1
                Layout.bottomMargin: 1
            }

            TrayMenuItem {
                action: root.quitAction
                onClicked: root.visible = false
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
