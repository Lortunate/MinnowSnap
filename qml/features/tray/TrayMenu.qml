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
    width: 130
    height: layout.height + 10
    color: "transparent"

    function popup(geometry) {
        if (!geometry)
            return;

        var screenW = Screen.width;
        var screenH = Screen.height;

        var iconCenterX = geometry.x + (geometry.width / 2);
        var targetX = iconCenterX - (width / 2);
        var targetY = geometry.y + geometry.height + 5;

        if (geometry.y > screenH / 2) {
            targetY = geometry.y - height - 5;
        }

        if (targetX < 5)
            targetX = 5;
        if (targetX + width > screenW - 5)
            targetX = screenW - width - 5;

        if (targetY < 5)
            targetY = 5;
        if (targetY + height > screenH - 5)
            targetY = screenH - height - 5;

        x = targetX;
        y = targetY;

        show();
        raise();
        requestActivate();
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
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: AppTheme.spacingTiny
            spacing: 1

            TrayMenuItem {
                action: root.preferencesAction
                onClicked: root.visible = false
            }

            Divider {
                Layout.topMargin: 1
                Layout.bottomMargin: 1
            }

            TrayMenuItem {
                action: root.quickCaptureAction
                onClicked: root.visible = false
            }

            TrayMenuItem {
                action: root.screenCaptureAction
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

        background: Rectangle {
            implicitHeight: AppTheme.buttonHeight
            color: control.hovered ? AppTheme.itemHover : "transparent"
            radius: AppTheme.radiusSmall
        }

        contentItem: Text {
            text: control.text
            color: AppTheme.text
            font.family: AppTheme.fontFamily
            font.pixelSize: AppTheme.fontSizeBody
            verticalAlignment: Text.AlignVCenter
            leftPadding: 12
            rightPadding: AppTheme.spacingSmall
            elide: Text.ElideRight
        }
    }
}
