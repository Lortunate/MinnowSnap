import QtQuick
import com.lortunate.minnow

AnnotationBase {
    id: root

    readonly property string type: "counter"

    // Properties specific to Counter
    property int number: 1
    property int size: 32

    // Config
    resizable: false // No handles, uses wheel
    draggable: true

    width: size
    height: size
    x: p1.x - size / 2
    y: p1.y - size / 2

    // Override handleResize
    function handleResize(delta) {
        // Delta is +/- 1. We scale it up.
        var step = delta > 0 ? 4 : -4;
        root.size = Math.max(16, Math.min(64, root.size + step));
    }

    Rectangle {
        anchors.fill: parent
        border.color: root.hasOutline ? root.color : (root.hasStroke ? "white" : "transparent")
        border.width: root.hasOutline ? 2 : (root.hasStroke ? 2 : 0)
        color: root.hasOutline ? "transparent" : root.color
        radius: width / 2

        Text {
            anchors.centerIn: parent
            color: root.hasOutline ? root.color : ((root.color.r * 0.299 + root.color.g * 0.587 + root.color.b * 0.114) > 0.6 ? "black" : "white")
            font.bold: true
            font.pixelSize: root.size * 0.5
            horizontalAlignment: Text.AlignHCenter
            text: root.number.toString()
            verticalAlignment: Text.AlignVCenter
        }
    }
}
