import QtQuick
import QtQuick.Layouts
import com.lortunate.minnow

Rectangle {
    id: root

    default property alias content: columnLayout.data

    Layout.fillWidth: true
    implicitHeight: columnLayout.implicitHeight
    height: implicitHeight

    color: AppTheme.cardBackground
    radius: AppTheme.radiusXLarge
    border.color: AppTheme.cardBorder
    border.width: 1
    clip: true

    ColumnLayout {
        id: columnLayout
        width: parent.width
        spacing: 0
    }
}
