import QtQuick
import QtQuick.Window
import QtQuick.Effects
import QtQuick.Layouts
import com.lortunate.minnow

Window {
    id: contextMenu

    property var menuItems: []
    property int itemHeight: 28
    property int padding: 5

    width: 120
    height: menuColumn.height + padding * 2
    visible: false
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "transparent"

    onActiveChanged: {
        if (!active) {
            visible = false;
        }
    }

    function popup(x, y) {
        contextMenu.x = x;
        contextMenu.y = y;
        contextMenu.show();
        contextMenu.requestActivate();
    }

    // Background & Shadow
    Rectangle {
        anchors.fill: parent
        color: AppTheme.surface
        border.color: AppTheme.border
        border.width: 1
        radius: AppTheme.radiusLarge

        layer.enabled: true
        layer.effect: MultiEffect {
            shadowEnabled: true
            shadowBlur: 8
            shadowColor: "#000000"
            shadowOpacity: 0.2
        }
    }

    // Menu Items
    Column {
        id: menuColumn
        width: parent.width
        topPadding: contextMenu.padding
        spacing: 2

        Repeater {
            model: contextMenu.menuItems

            delegate: Item {
                width: contextMenu.width
                height: modelData.isDivider ? AppTheme.spacingSmall : contextMenu.itemHeight

                Rectangle {
                    width: parent.width - AppTheme.spacingSmall
                    height: contextMenu.itemHeight
                    anchors.horizontalCenter: parent.horizontalCenter
                    visible: !modelData.isDivider
                    color: itemMouseArea.containsMouse && modelData.enabled !== false ? AppTheme.primary : "transparent"
                    radius: AppTheme.radiusSmall

                    Text {
                        anchors.fill: parent
                        anchors.leftMargin: AppTheme.spacingSmall
                        verticalAlignment: Text.AlignVCenter
                        text: modelData.text || ""
                        color: {
                            if (modelData.enabled === false) {
                                return AppTheme.subText;
                            }
                            return itemMouseArea.containsMouse ? AppTheme.primaryText : AppTheme.text;
                        }
                        font.family: AppTheme.fontFamily
                        font.pixelSize: AppTheme.fontSizeBody
                    }

                    MouseArea {
                        id: itemMouseArea
                        anchors.fill: parent
                        hoverEnabled: true
                        enabled: modelData.enabled !== false

                        onClicked: {
                            contextMenu.visible = false;
                            if (modelData.action) {
                                modelData.action();
                            }
                        }
                    }
                }

                // Divider
                Item {
                    width: parent.width
                    height: AppTheme.spacingSmall
                    visible: modelData.isDivider === true
                    anchors.horizontalCenter: parent.horizontalCenter

                    Rectangle {
                        width: parent.width
                        height: 1
                        color: AppTheme.divider
                        opacity: 0.5
                        anchors.centerIn: parent
                    }
                }
            }
        }
    }
}
