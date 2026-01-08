import QtQuick
import QtQuick.Layouts
import com.lortunate.minnow

Item {
    id: root

    property string text: ""
    property string url: ""
    property bool showChevron: true

    signal clicked

    Layout.fillWidth: true
    Layout.preferredHeight: 48

    Rectangle {
        anchors.fill: parent
        color: mouseArea.containsMouse ? AppTheme.itemHover : "transparent"
        radius: AppTheme.radiusXLarge

        Behavior on color {
            ColorAnimation {
                duration: AppTheme.durationFast
            }
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: AppTheme.spacingMedium
        anchors.rightMargin: AppTheme.spacingMedium
        spacing: AppTheme.spacingSmall

        Text {
            Layout.fillWidth: true
            text: root.text
            color: AppTheme.text
            font.family: AppTheme.fontFamily
            font.pixelSize: AppTheme.fontSizeBody
        }

        Text {
            visible: root.showChevron
            text: "›"
            color: AppTheme.subText
            font.pixelSize: AppTheme.fontSizeLarge
            opacity: 0.5
        }
    }

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor

        onClicked: {
            if (root.url !== "") {
                Qt.openUrlExternally(root.url);
            }
            root.clicked();
        }
    }
}
