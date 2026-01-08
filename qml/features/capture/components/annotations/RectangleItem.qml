import QtQuick
import com.lortunate.minnow

AnnotationBase {
    id: root

    readonly property string type: "rectangle"

    // Config
    padding: Math.max(12, lineWidth * 2)
    maintainAspectRatio: true
    resizable: true

    x: Math.min(p1.x, p2.x) - padding
    y: Math.min(p1.y, p2.y) - padding
    width: Math.abs(p1.x - p2.x) + (padding * 2)
    height: Math.abs(p1.y - p2.y) + (padding * 2)

    // Override isHit
    function isHit(mx, my) {
        var threshold = 10;
        var l = Math.min(localP1.x, localP2.x);
        var r = Math.max(localP1.x, localP2.x);
        var t = Math.min(localP1.y, localP2.y);
        var b = Math.max(localP1.y, localP2.y);

        // If filled, hit if inside
        if (!hasOutline) {
            return mx >= l && mx <= r && my >= t && my <= b;
        }

        // If outline, hit if near border
        var d1 = Math.abs(my - t); // Top
        var d2 = Math.abs(my - b); // Bottom
        var d3 = Math.abs(mx - l); // Left
        var d4 = Math.abs(mx - r); // Right

        var onH = (mx >= l - threshold && mx <= r + threshold);
        var onV = (my >= t - threshold && my <= b + threshold);

        if (onH && (d1 <= threshold || d2 <= threshold))
            return true;
        if (onV && (d3 <= threshold || d4 <= threshold))
            return true;

        return false;
    }

    Rectangle {
        x: root.padding
        y: root.padding
        width: Math.abs(p1.x - p2.x)
        height: Math.abs(p1.y - p2.y)
        color: root.hasOutline ? "transparent" : root.color
        border.color: root.hasOutline ? root.color : (root.hasStroke ? "white" : "transparent")
        border.width: root.hasOutline ? root.lineWidth : (root.hasStroke ? 2 : 0)
    }
}
