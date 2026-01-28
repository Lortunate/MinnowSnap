import QtQuick
import com.lortunate.minnow

AnnotationBase {
    id: root

    readonly property string type: "text"

    property int fontSize: 24
    property bool editing: false
    property string textContent: "Text"

    readonly property color textColor: !root.hasOutline ? ((root.color.r * 0.299 + root.color.g * 0.587 + root.color.b * 0.114) > 0.6 ? "black" : "white") : root.color

    resizable: false
    draggable: !editing && !drawingMode
    maintainAspectRatio: true

    width: (root.editing ? textInput.width : textDisplay.width) + 16
    height: (root.editing ? textInput.height : textDisplay.height) + 16
    x: p1.x
    y: p1.y

    function handleResize(delta) {
        var step = delta > 0 ? 2 : -2;
        root.fontSize = Math.max(12, Math.min(96, root.fontSize + step));
    }

    function isHit(mx, my) {
        if (drawingMode) {
            return false;
        }
        return mx >= 0 && mx <= width && my >= 0 && my <= height;
    }

    onSelectedChanged: {
        if (!selected) {
            editing = false;
            if (root.textContent.trim() === "") {
                root.requestRemove();
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        border.color: root.selected ? AppTheme.primary : (root.hasStroke && !root.hasOutline && !root.editing ? "white" : "transparent")
        border.width: root.selected || (root.hasStroke && !root.hasOutline) ? 1 : 0
        color: root.editing ? "#80000000" : (!root.hasOutline ? root.color : "transparent")
        radius: AppTheme.radiusSmall

        Canvas {
            anchors.fill: parent
            visible: root.editing
            onPaint: {
                var ctx = getContext("2d");
                ctx.strokeStyle = AppTheme.primary;
                ctx.lineWidth = 1;
                ctx.setLineDash([4, 2]);
                ctx.beginPath();
                ctx.rect(0, 0, width, height);
                ctx.stroke();
            }
        }
    }

    Text {
        id: textDisplay

        anchors.centerIn: parent
        text: root.textContent
        color: root.textColor
        font.family: AppTheme.fontFamily
        font.pixelSize: root.fontSize
        visible: !root.editing

        style: Text.Normal
    }

    TextEdit {
        id: textInput

        anchors.centerIn: parent
        color: root.textColor
        font.family: AppTheme.fontFamily
        font.pixelSize: root.fontSize
        visible: root.editing
        text: root.textContent
        selectByMouse: true
        focus: root.editing

        onTextChanged: root.textContent = text

        Keys.onPressed: event => {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                if (event.modifiers & Qt.ShiftModifier) {
                    insert(cursorPosition, "\n");
                } else {
                    root.editing = false;
                    root.interactionEnded();
                    event.accepted = true;
                }
            } else if (event.key === Qt.Key_Escape) {
                root.editing = false;
                root.interactionEnded();
                event.accepted = true;
            }
        }

        Component.onCompleted: {
            if (root.selected) {
                root.editing = true;
                textInput.forceActiveFocus();
                textInput.selectAll();
            }
        }

        onEditingFinished: {
            root.editing = false;
        }
    }

    Connections {
        target: root.mouseArea

        function onDoubleClicked(mouse) {
            root.editing = true;
            textInput.forceActiveFocus();
        }
    }
}
