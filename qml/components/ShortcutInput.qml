import QtQuick
import com.lortunate.minnow

Rectangle {
    id: root

    property string defaultValue: ""
    property bool hasConflict: false
    property bool isRecording: false

    property string sequence: ""
    signal committed(string newSequence)

    border.color: hasConflict ? AppTheme.danger : (root.isRecording ? AppTheme.primary : AppTheme.border)
    border.width: 1

    color: root.isRecording ? AppTheme.surface : AppTheme.background
    implicitHeight: AppTheme.inputHeight
    implicitWidth: Math.min(200, Math.max(shortcutText.implicitWidth + AppTheme.spacingMedium * 2, 100))
    radius: AppTheme.radiusMedium

    Behavior on border.color {
        ColorAnimation {
            duration: AppTheme.durationNormal
        }
    }

    Keys.onPressed: event => {
        if (!isRecording)
            return;

        if (event.key === Qt.Key_Backspace || event.key === Qt.Key_Delete) {
            root.isRecording = false;
            root.committed("");
            event.accepted = true;
            return;
        }

        const result = helper.getKeySequence(event.key, event.modifiers);

        if (result !== "") {
            root.isRecording = false;
            root.committed(result);
        }

        event.accepted = true;
    }
    onActiveFocusChanged: {
        if (!activeFocus && isRecording)
            isRecording = false;
    }

    ShortcutHelper {
        id: helper
    }

    Rectangle {
        anchors.fill: parent
        anchors.topMargin: 1
        color: "#000000"
        opacity: 0.05
        radius: AppTheme.radiusMedium
        visible: !AppTheme.isDark && !root.isRecording
        z: -1
    }
    Text {
        id: shortcutText

        anchors.fill: parent
        anchors.leftMargin: AppTheme.spacingSmall
        anchors.rightMargin: AppTheme.spacingSmall
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight

        color: root.isRecording ? AppTheme.primary : AppTheme.text
        font.family: AppTheme.fontFamilyMono
        font.pixelSize: AppTheme.fontSizeSmall
        font.weight: Font.Medium
        text: root.isRecording ? qsTr("Press keys...") : (root.sequence === "" ? root.defaultValue : root.sequence)
    }
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true

        onClicked: {
            root.isRecording = !root.isRecording;
            if (root.isRecording)
                root.forceActiveFocus();
        }
    }
}
