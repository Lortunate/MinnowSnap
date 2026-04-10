import QtQuick
import QtQuick.Layouts
import com.lortunate.minnow

Item {
    id: root

    property string title: ""
    property string description: ""
    property alias control: controlLoader.sourceComponent

    Layout.fillWidth: true
    // Automatically adjust height based on the content (title + description) or theme defaults
    implicitHeight: Math.max(description !== "" ? AppTheme.settingItemHeightTall : AppTheme.settingItemHeight, contentLayout.implicitHeight + AppTheme.spacingMedium * 2)

    RowLayout {
        id: contentLayout
        anchors.fill: parent
        anchors.margins: AppTheme.spacingMedium
        spacing: AppTheme.spacingMedium

        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            spacing: 4

            Text {
                text: root.title
                color: AppTheme.text
                font.family: AppTheme.fontFamily
                font.pixelSize: AppTheme.fontSizeBody
                font.weight: Font.Medium
                Layout.fillWidth: true
                elide: Text.ElideRight
            }

            Text {
                visible: root.description !== ""
                text: root.description
                color: AppTheme.subText
                font.family: AppTheme.fontFamily
                font.pixelSize: AppTheme.fontSizeSmall
                opacity: 0.7
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }

        Loader {
            id: controlLoader
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            // Explicitly notify the RowLayout of the control's preferred width
            // to ensure the text ColumnLayout correctly calculates its remaining space.
            Layout.preferredWidth: item ? item.implicitWidth : 0
        }
    }
}
