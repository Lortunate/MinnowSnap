import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts
import com.lortunate.minnow

Basic.ComboBox {
    id: control

    property bool previewFonts: false
    property real maxItemWidth: 0

    implicitWidth: Math.max(120, Math.min(320, maxItemWidth + 48))
    implicitHeight: 28

    leftPadding: 10
    rightPadding: 30
    spacing: 6

    function updateMaxWidth() {
        if (!model)
            return;

        let count = 0;
        try {
            if (Array.isArray(model))
                count = model.length;
            else if (model.count !== undefined)
                count = model.count;
            else if (typeof model.length === "number")
                count = model.length;
        } catch (e) {
            return;
        }

        if (count <= 0)
            return;

        let maxWidth = 0;
        let limit = Math.min(count, 100);
        let measurer = Qt.createQmlObject('import QtQuick; Text { font.pixelSize: 13; visible: false }', control);

        for (let i = 0; i < limit; ++i) {
            let t = "";
            try {
                if (Array.isArray(model)) {
                    t = model[i];
                } else if (typeof model.get === "function") {
                    let obj = model.get(i);
                    t = (typeof obj === "string") ? obj : (obj.display || obj.text || "");
                } else {
                    t = model[i] || "";
                }
            } catch (e) {
                continue;
            }

            if (control.previewFonts && t !== "") {
                measurer.font.family = t;
            } else {
                measurer.font.family = AppTheme.fontFamily;
            }

            measurer.text = t;
            maxWidth = Math.max(maxWidth, measurer.contentWidth);
        }

        measurer.destroy();
        maxItemWidth = maxWidth;
    }

    onModelChanged: updateMaxWidth()
    Component.onCompleted: updateMaxWidth()

    background: Rectangle {
        border.color: AppTheme.border
        border.width: 1
        color: control.down ? AppTheme.itemPress : (control.hovered ? AppTheme.itemHover : "transparent")
        radius: AppTheme.radiusMedium

        Behavior on color {
            ColorAnimation {
                duration: 150
            }
        }
    }

    contentItem: Text {
        color: AppTheme.text
        elide: Text.ElideRight
        font.family: control.previewFonts ? control.displayText : AppTheme.fontFamily
        font.pixelSize: 13
        text: control.displayText
        verticalAlignment: Text.AlignVCenter
    }

    delegate: ItemDelegate {
        id: delegateItem
        width: control.popup.width
        height: 28
        padding: 0

        background: Rectangle {
            anchors.fill: parent
            anchors.leftMargin: 4
            anchors.rightMargin: 4
            anchors.topMargin: 1
            anchors.bottomMargin: 1
            color: delegateItem.highlighted ? AppTheme.primary : (delegateItem.hovered ? AppTheme.itemHover : "transparent")
            radius: AppTheme.radiusSmall
        }

        contentItem: Text {
            color: delegateItem.highlighted ? AppTheme.primaryText : AppTheme.text
            elide: Text.ElideRight
            font.family: control.previewFonts ? modelData : AppTheme.fontFamily
            font.pixelSize: 13
            font.weight: Font.Normal
            leftPadding: 10
            rightPadding: 10
            text: modelData
            verticalAlignment: Text.AlignVCenter
        }
    }

    indicator: Item {
        x: control.width - width - 8
        y: (control.height - height) / 2
        width: 12
        height: 12

        Image {
            id: arrowIcon
            anchors.fill: parent
            source: "qrc:/resources/icons/arrow_drop_down.svg"
            sourceSize: Qt.size(12, 12)
            smooth: true
            opacity: control.opened ? 1.0 : 0.5
            rotation: control.opened ? 180 : 0

            Behavior on rotation {
                NumberAnimation {
                    duration: 150
                    easing.type: Easing.InOutQuad
                }
            }
            Behavior on opacity {
                NumberAnimation {
                    duration: 150
                }
            }
        }
    }

    popup: Popup {
        id: comboPopup
        y: control.height + 4
        width: control.width
        implicitHeight: Math.min(listView.contentHeight + 8, 300)
        padding: 4
        transformOrigin: Item.Top

        enter: Transition {
            NumberAnimation {
                property: "opacity"
                from: 0.0
                to: 1.0
                duration: 150
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                property: "scale"
                from: 0.98
                to: 1.0
                duration: 150
                easing.type: Easing.OutCubic
            }
        }

        exit: Transition {
            NumberAnimation {
                property: "opacity"
                from: 1.0
                to: 0.0
                duration: 150
                easing.type: Easing.InCubic
            }
            NumberAnimation {
                property: "scale"
                from: 1.0
                to: 0.98
                duration: 150
                easing.type: Easing.InCubic
            }
        }

        background: Rectangle {
            border.color: AppTheme.border
            border.width: 1
            color: AppTheme.cardBackground
            radius: AppTheme.radiusLarge

            Rectangle {
                anchors.fill: parent
                z: -1
                anchors.margins: -1
                radius: AppTheme.radiusLarge
                color: "transparent"
                border.color: AppTheme.isDark ? "transparent" : AppTheme.shadowLight
                border.width: 1
            }
        }

        contentItem: ListView {
            id: listView
            clip: true
            currentIndex: control.highlightedIndex
            model: control.popup.visible ? control.delegateModel : null
            boundsBehavior: Flickable.StopAtBounds

            ScrollBar.vertical: Basic.ScrollBar {
                id: scrollBar
                width: 6
                policy: ScrollBar.AsNeeded

                contentItem: Rectangle {
                    implicitWidth: 6
                    radius: 3
                    color: AppTheme.text
                    opacity: (scrollBar.active || scrollBar.hovered) ? 0.4 : 0.0
                    Behavior on opacity {
                        NumberAnimation {
                            duration: 200
                        }
                    }
                }
            }
        }
    }
}
