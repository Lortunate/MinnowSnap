import QtQuick
import QtQuick.Window
import com.lortunate.minnow
import "components"
import "../../components"

Window {
    id: root

    property string busyText: qsTr("Processing...")
    property bool isBusy: false
    property rect selectionRect: Qt.rect(0, 0, 0, 0)
    property rect viewportRect: Qt.rect(0, 0, 0, 0)

    signal actionClicked(int action)

    color: "transparent"

    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool
    height: isBusy ? busyRect.height : toolbar.height
    width: isBusy ? busyRect.width : toolbar.width

    x: {
        let desiredX = selectionRect.x + selectionRect.width - width
        let maxX = Math.max(10, viewportRect.width - width - 10)
        let localX = Math.max(10, Math.min(maxX, desiredX))
        return viewportRect.x + localX
    }
    y: {
        let desiredY = selectionRect.y + selectionRect.height + 8
        let topY = selectionRect.y - height - 8
        let localY = (desiredY + height <= viewportRect.height) ? desiredY : (topY >= 0 ? topY : 40)
        return viewportRect.y + localY
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
