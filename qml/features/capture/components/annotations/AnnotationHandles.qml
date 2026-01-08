import QtQuick
import com.lortunate.minnow

Item {
    id: root

    property bool showBoundingBox: false
    // Target item must have: p1, p2, localP1, localP2, requestRemove(), requestSelect(), interactionStarted(), interactionEnded()
    required property var target

    anchors.fill: parent
    visible: target.selected

    // P1 Handle
    Rectangle {
        id: p1Handle

        border.color: AppTheme.primary
        border.width: 2
        color: "white"
        height: AppTheme.annotationHandleSize
        radius: 6
        width: AppTheme.annotationHandleSize
        x: target.localP1.x - 6
        y: target.localP1.y - 6

        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: {
                // Determine cursor based on relative position
                var dx = target.p1.x - target.p2.x;
                var dy = target.p1.y - target.p2.y;
                // If signs match (both pos or both neg), it's NW-SE (FDiag)
                // If signs differ, it's NE-SW (BDiag)
                if ((dx > 0 && dy > 0) || (dx < 0 && dy < 0))
                    return Qt.SizeFDiagCursor;
                return Qt.SizeBDiagCursor;
            }

            onPositionChanged: mouse => {
                if (mouse.buttons === Qt.NoButton)
                    return;
                var pt = mapToItem(target.parent, mouse.x, mouse.y);

                // Shift Constraint
                if (mouse.modifiers & Qt.ShiftModifier) {
                    // Check if target supports aspect ratio constraint (Circle/Rect)
                    // We can check simply if we are in Circle or Rect mode?
                    // Or generically: if target object behaves like a box.
                    // Let's assume Shift is always "Maintain Aspect Ratio" for 2-point sizing items

                    var fixed = target.p2;
                    var dx = pt.x - fixed.x;
                    var dy = pt.y - fixed.y;

                    // Only apply if it makes sense (not for Arrow which is a line)
                    // We can infer type or pass property.
                    // Simple heuristic: if showBoundingBox is true, we enforce aspect ratio.
                    // If it's false (Arrow), we don't.
                    if (root.showBoundingBox) {
                        var size = Math.max(Math.abs(dx), Math.abs(dy));
                        var sx = dx >= 0 ? 1 : -1;
                        var sy = dy >= 0 ? 1 : -1;
                        pt = Qt.point(fixed.x + sx * size, fixed.y + sy * size);
                    }
                }

                target.p1 = pt;
            }
            onPressed: mouse => {
                target.requestSelect();
                target.interactionStarted();
                mouse.accepted = true;
            }
            onReleased: target.interactionEnded()
        }
    }

    // P2 Handle
    Rectangle {
        id: p2Handle

        border.color: AppTheme.primary
        border.width: 2
        color: "white"
        height: AppTheme.annotationHandleSize
        radius: 6
        width: AppTheme.annotationHandleSize
        x: target.localP2.x - 6
        y: target.localP2.y - 6

        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: {
                var dx = target.p2.x - target.p1.x;
                var dy = target.p2.y - target.p1.y;
                if ((dx > 0 && dy > 0) || (dx < 0 && dy < 0))
                    return Qt.SizeFDiagCursor;
                return Qt.SizeBDiagCursor;
            }

            onPositionChanged: mouse => {
                if (mouse.buttons === Qt.NoButton)
                    return;
                var pt = mapToItem(target.parent, mouse.x, mouse.y);

                if (mouse.modifiers & Qt.ShiftModifier) {
                    var fixed = target.p1;
                    var dx = pt.x - fixed.x;
                    var dy = pt.y - fixed.y;

                    if (root.showBoundingBox) {
                        var size = Math.max(Math.abs(dx), Math.abs(dy));
                        var sx = dx >= 0 ? 1 : -1;
                        var sy = dy >= 0 ? 1 : -1;
                        pt = Qt.point(fixed.x + sx * size, fixed.y + sy * size);
                    }
                }

                target.p2 = pt;
            }
            onPressed: mouse => {
                target.requestSelect();
                target.interactionStarted();
                mouse.accepted = true;
            }
            onReleased: target.interactionEnded()
        }
    }
}
