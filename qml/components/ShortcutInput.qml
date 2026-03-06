import QtQuick
import QtQuick.Controls.Basic
import com.lortunate.minnow

Rectangle {
    id: root

    property string defaultValue: ""
    property bool hasConflict: false
    property bool isRecording: false

    property string sequence: ""
    signal committed(string newSequence)

    readonly property var keyTokens: {
        if (!root.sequence) return [];
        return root.sequence.split("+");
    }

    implicitWidth: Math.max(160, contentRow.implicitWidth + 40)
    implicitHeight: Math.max(AppTheme.inputHeight + 4, 32)
    
    color: isRecording ? AppTheme.inputRecordingBg : AppTheme.inputFill
    radius: AppTheme.radiusMedium
    border.width: isRecording || hasConflict ? 2 : 1
    border.color: {
        if (hasConflict) return AppTheme.danger;
        if (isRecording) return AppTheme.primary;
        return activeFocus || hoverHandler.hovered ? AppTheme.primary : AppTheme.border;
    }

    Behavior on border.color { ColorAnimation { duration: 200 } }
    Behavior on color { ColorAnimation { duration: 200 } }

    ShortcutHelper { id: helper }

    HoverHandler {
        id: hoverHandler
    }

    Keys.onPressed: event => {
        if (!isRecording) return;
        
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
        if (!activeFocus && isRecording) {
            isRecording = false;
        }
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: {
            root.isRecording = !root.isRecording;
            if (root.isRecording) root.forceActiveFocus();
        }
    }

    Item {
        id: contentItem
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: clearBtn.visible ? 36 : 12

        Row {
            id: contentRow
            anchors.centerIn: parent
            spacing: 4
            
            Text {
                visible: root.isRecording
                text: qsTr("Press keys...")
                color: AppTheme.primary
                font.pixelSize: AppTheme.fontSizeSmall
                font.family: AppTheme.fontFamily
                font.bold: true
                anchors.verticalCenter: parent.verticalCenter
            }

            Text {
                visible: !root.isRecording && root.keyTokens.length === 0
                text: qsTr("None")
                color: AppTheme.subText
                font.pixelSize: AppTheme.fontSizeSmall
                font.family: AppTheme.fontFamily
                anchors.verticalCenter: parent.verticalCenter
            }

            Repeater {
                model: !root.isRecording ? root.keyTokens : []
                delegate: Item {
                    height: AppTheme.keyCapHeight + AppTheme.keyCapShadowHeight
                    width: keyLabel.implicitWidth + 16
                    anchors.verticalCenter: parent.verticalCenter
                    
                    Rectangle {
                        anchors.fill: parent
                        anchors.topMargin: AppTheme.keyCapShadowHeight
                        radius: AppTheme.keyCapRadius
                        color: AppTheme.keyCapShadow
                    }
                    
                    Rectangle {
                        anchors.fill: parent
                        anchors.bottomMargin: AppTheme.keyCapShadowHeight
                        color: AppTheme.keyCapBackground
                        radius: AppTheme.keyCapRadius
                        border.color: AppTheme.keyCapBorder
                        border.width: 1
                        
                        Text {
                            id: keyLabel
                            anchors.centerIn: parent
                            text: modelData
                            font.family: AppTheme.fontFamilyMono
                            font.pixelSize: 11
                            font.bold: true
                            color: AppTheme.text
                        }
                    }
                }
            }
        }
    }

    SequentialAnimation {
        running: root.isRecording
        loops: Animation.Infinite
        PropertyAnimation {
            target: root
            property: "color"
            from: AppTheme.inputRecordingBg
            to: AppTheme.inputRecordingPulse
            duration: 800
            easing.type: Easing.InOutQuad
        }
        PropertyAnimation {
            target: root
            property: "color"
            from: AppTheme.inputRecordingPulse
            to: AppTheme.inputRecordingBg
            duration: 800
            easing.type: Easing.InOutQuad
        }
    }

    Button {
        id: clearBtn
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.rightMargin: 6
        width: 24
        height: 24
        
        visible: !root.isRecording && root.sequence !== "" && (hoverHandler.hovered || hovered)
        opacity: visible ? 1.0 : 0.0
        Behavior on opacity { NumberAnimation { duration: 150 } }

        background: Rectangle {
            radius: 12
            color: parent.hovered ? AppTheme.iconButtonHover : "transparent"
        }

        icon.source: "qrc:/resources/icons/close.svg"
        icon.width: 12
        icon.height: 12
        icon.color: AppTheme.text
        
        onClicked: root.committed("")
    }
}
