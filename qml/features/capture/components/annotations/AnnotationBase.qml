import QtQuick
import com.lortunate.minnow

Item {
    id: root

    property color color: AppTheme.danger
    property bool hasOutline: true
    property bool hasStroke: false
    property int lineWidth: 4
    property bool selected: false

    // Geometry
    property point p1: Qt.point(0, 0)
    property point p2: Qt.point(0, 0)

    // Computed properties for handles and drawing
    readonly property point localP1: Qt.point(p1.x - x, p1.y - y)
    readonly property point localP2: Qt.point(p2.x - x, p2.y - y)

    // Configuration
    property bool draggable: true
    property bool resizable: true // Whether to show handles
    property bool maintainAspectRatio: false // For handles
    property int padding: 0 // For bounding box calculation

    signal interactionEnded
    signal interactionStarted
    signal requestRemove
    signal requestSelect

    function isHit(mx, my) {
        return true;
    }

    function handleResize(delta) {
        root.lineWidth = Math.max(1, Math.min(50, root.lineWidth + delta));
    }

    property alias mouseArea: dragArea
    MouseArea {
        id: dragArea

        property point dragStartParent: Qt.point(0, 0)

        anchors.fill: parent
        enabled: root.draggable
        hoverEnabled: true

        cursorShape: {
            if (root.isHit(mouseX, mouseY)) {
                return root.selected ? Qt.SizeAllCursor : Qt.PointingHandCursor;
            }
            return Qt.ArrowCursor;
        }

        onPositionChanged: mouse => {
            if (pressed) {
                var currentParent = mapToItem(root.parent, mouse.x, mouse.y);
                var dx = currentParent.x - dragStartParent.x;
                var dy = currentParent.y - dragStartParent.y;

                root.p1 = Qt.point(root.p1.x + dx, root.p1.y + dy);

                if (root.type !== "counter" && root.type !== "text") {
                    root.p2 = Qt.point(root.p2.x + dx, root.p2.y + dy);
                }

                dragStartParent = currentParent;
            }
        }

        onPressed: mouse => {
            if (!root.isHit(mouse.x, mouse.y)) {
                mouse.accepted = false;
                return;
            }

            root.requestSelect();
            root.selected = true;
            root.interactionStarted();
            dragStartParent = mapToItem(root.parent, mouse.x, mouse.y);
            mouse.accepted = true;
        }

        onReleased: root.interactionEnded()

        onWheel: wheel => {
            if (root.selected) {
                var delta = wheel.angleDelta.y > 0 ? 1 : -1;
                root.handleResize(delta);
                wheel.accepted = true;
            } else {
                wheel.accepted = false;
            }
        }
    }

    AnnotationHandles {
        target: root
        showBoundingBox: root.maintainAspectRatio
        visible: root.selected && root.resizable
    }
}
