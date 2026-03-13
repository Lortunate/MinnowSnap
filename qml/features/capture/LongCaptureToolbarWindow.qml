import QtQuick
import QtQuick.Window
import com.lortunate.minnow
import "components"
import "../../components"

Window {
    id: root

    property string busyText: qsTr("Processing...")
    property bool isBusy: false
    property int selectionHeight: 0
    property int selectionWidth: 0
    property int selectionX: 0
    property int selectionY: 0

    signal actionClicked(int action)

    color: "transparent"

    // Interactable window
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool
    height: isBusy ? busyRect.height : toolbar.height

    // Size logic
    width: isBusy ? busyRect.width : toolbar.width

    // Position logic
    x: {
        let desiredX = selectionX + selectionWidth - width;
        let sW = Screen.width;
        return Math.max(10, Math.min(sW - width - 10, desiredX));
    }
    y: {
        let desiredY = selectionY + selectionHeight + 8;
        let topY = selectionY - height - 8;
        let sH = Screen.height;
        return (desiredY + height <= sH) ? desiredY : (topY >= 0 ? topY : 40);
    }

    SelectionToolbar {
        id: toolbar

        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        showTooltips: false
        buttons: [[
                {
                    "icon": "qrc:/resources/icons/save.svg",
                    "text": qsTr("Save"),
                    "action": CaptureActions.Save,
                    "hoverColor": AppTheme.primary
                },
                {
                    "icon": "qrc:/resources/icons/keep.svg",
                    "text": qsTr("Pin"),
                    "action": CaptureActions.Pin,
                    "hoverColor": AppTheme.primary
                },
                {
                    "icon": "qrc:/resources/icons/file_copy.svg",
                    "text": qsTr("Copy"),
                    "action": CaptureActions.Copy,
                    "hoverColor": AppTheme.primary
                }
            ], [
                {
                    "icon": "qrc:/resources/icons/close.svg",
                    "text": qsTr("Cancel"),
                    "action": CaptureActions.Cancel,
                    "hoverColor": AppTheme.danger
                }
            ]]
        visible: !isBusy

        onActionConfirmed: action => root.actionClicked(action)
        onCanceled: root.actionClicked(CaptureActions.Cancel)
    }
    BusyStatus {
        id: busyRect

        anchors.centerIn: parent
        running: isBusy
        text: root.busyText
    }
}
