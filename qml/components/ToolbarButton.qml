import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Controls.Basic as Basic
import com.lortunate.minnow
import "."

Basic.Button {
    id: root

    property color defaultIconColor: AppTheme.icon
    property color hoveredIconColor: defaultIconColor
    property bool isActive: false
    property string tooltipText: ""

    Layout.preferredHeight: AppTheme.buttonHeight
    Layout.preferredWidth: AppTheme.buttonHeight
    display: AbstractButton.IconOnly
    icon.color: isActive ? hoveredIconColor : (hovered ? hoveredIconColor : defaultIconColor)
    icon.height: 18
    icon.width: 18

    background: Rectangle {
        color: root.down ? AppTheme.itemPress : (root.isActive ? AppTheme.itemSelected : (root.hovered ? AppTheme.itemHover : "transparent"))
        radius: AppTheme.radiusMedium

        Behavior on color {
            ColorAnimation {
                duration: AppTheme.durationFast
            }
        }
    }

    Tooltip {
        parent: root
        text: root.tooltipText
        visible: root.hovered && root.tooltipText !== ""
        position: Qt.AlignTop
    }
}
