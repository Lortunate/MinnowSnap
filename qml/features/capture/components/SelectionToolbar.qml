import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.lortunate.minnow
import "../../../components"

Rectangle {
    id: root

    property string activeTool: ""
    property bool showTooltips: true

    readonly property int toolbarHeight: 36
    readonly property int toolbarPadding: 12
    readonly property int toolbarRowSpacing: 4
    readonly property int buttonSize: 32
    readonly property int iconSize: 20

    property var buttons: [[
            {
                "icon": "qrc:/resources/icons/square.svg",
                "text": qsTr("Rectangle"),
                "tool": "rectangle",
                "isTool": true,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/circle.svg",
                "text": qsTr("Circle"),
                "tool": "circle",
                "isTool": true,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/counter_1.svg",
                "text": qsTr("Counter"),
                "tool": "counter",
                "isTool": true,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/arrow_insert.svg",
                "text": qsTr("Arrow"),
                "tool": "arrow",
                "isTool": true,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/text_fields.svg",
                "text": qsTr("Text"),
                "tool": "text",
                "isTool": true,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/grid_on.svg",
                "text": qsTr("Mosaic"),
                "tool": "mosaic",
                "isTool": true,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/undo.svg",
                "text": qsTr("Undo"),
                "action": CaptureActions.Undo,
                "isTool": false,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/redo.svg",
                "text": qsTr("Redo"),
                "action": CaptureActions.Redo,
                "isTool": false,
                "hoverColor": AppTheme.primary
            }
        ], [
            {
                "icon": "qrc:/resources/icons/text_fields.svg",
                "text": qsTr("OCR"),
                "action": CaptureActions.Ocr,
                "isTool": false,
                "hoverColor": AppTheme.primary,
                "visible": Config.enableOcr
            },
            {
                "icon": "qrc:/resources/icons/crop_free.svg",
                "text": qsTr("QR Code"),
                "action": CaptureActions.QrCode,
                "isTool": false,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/scroll.svg",
                "text": qsTr("Scroll"),
                "action": CaptureActions.Scroll,
                "isTool": false,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/save.svg",
                "text": qsTr("Save"),
                "action": CaptureActions.Save,
                "isTool": false,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/keep.svg",
                "text": qsTr("Pin"),
                "action": CaptureActions.Pin,
                "isTool": false,
                "hoverColor": AppTheme.primary
            },
            {
                "icon": "qrc:/resources/icons/file_copy.svg",
                "text": qsTr("Copy"),
                "action": CaptureActions.Copy,
                "isTool": false,
                "hoverColor": AppTheme.primary
            }
        ], [
            {
                "icon": "qrc:/resources/icons/close.svg",
                "text": qsTr("Cancel"),
                "action": CaptureActions.Cancel,
                "isTool": false,
                "hoverColor": AppTheme.danger
            }
        ]]

    signal actionConfirmed(int action)
    signal canceled
    signal toolChanged(string tool)

    border.color: AppTheme.border
    border.width: 1
    color: AppTheme.surface
    height: toolbarHeight
    layer.enabled: true
    radius: AppTheme.radiusLarge
    width: toolbarRow.width + toolbarPadding

    MouseArea {
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        anchors.fill: parent
        cursorShape: Qt.ArrowCursor
        onPressed: mouse.accepted = true
    }

    RowLayout {
        id: toolbarRow
        anchors.centerIn: parent
        spacing: toolbarRowSpacing

        Repeater {
            model: root.buttons

            delegate: RowLayout {
                spacing: toolbarRowSpacing

                Repeater {
                    model: modelData

                    delegate: ToolbarButton {
                        Layout.preferredHeight: buttonSize
                        Layout.preferredWidth: buttonSize
                        hoveredIconColor: modelData.hoverColor
                        icon.height: iconSize
                        icon.source: modelData.icon
                        icon.width: iconSize
                        isActive: modelData.tool === root.activeTool
                        showTooltip: root.showTooltips
                        tooltipText: modelData.text
                        visible: modelData.visible === undefined ? true : modelData.visible

                        onClicked: {
                            if (modelData.action === CaptureActions.Cancel) {
                                root.canceled();
                            } else if (modelData.isTool) {
                                root.activeTool = root.activeTool === modelData.tool ? "" : modelData.tool;
                                root.toolChanged(root.activeTool);
                            } else {
                                root.actionConfirmed(modelData.action);
                            }
                        }
                    }
                }

                ToolbarSeparator {
                    visible: index < root.buttons.length - 1
                }
            }
        }
    }
}
