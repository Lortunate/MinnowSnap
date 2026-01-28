import QtQuick
import com.lortunate.minnow

AnnotationBase {
    id: root

    readonly property string type: "circle"

    padding: Math.max(12, lineWidth * 2)
    maintainAspectRatio: true
    resizable: true

    x: Math.min(p1.x, p2.x) - padding
    y: Math.min(p1.y, p2.y) - padding
    width: Math.abs(p1.x - p2.x) + (padding * 2)
    height: Math.abs(p1.y - p2.y) + (padding * 2)

    function isHit(mx, my) {
        if (drawingMode) {
            return false;
        }

        var threshold = 10;
        var l = Math.min(localP1.x, localP2.x);
        var r = Math.max(localP1.x, localP2.x);
        var t = Math.min(localP1.y, localP2.y);
        var b = Math.max(localP1.y, localP2.y);

        var w = r - l;
        var h = b - t;
        var cx = l + w / 2;
        var cy = t + h / 2;
        var rx = w / 2;
        var ry = h / 2;

        if (rx <= 0 || ry <= 0)
            return false;

        var dx = mx - cx;
        var dy = my - cy;

        if (!hasOutline) {
            return ((dx * dx) / (rx * rx) + (dy * dy) / (ry * ry)) <= 1.0;
        } else {
            var rx_out = rx + threshold;
            var ry_out = ry + threshold;
            var dist_out = (dx * dx) / (rx_out * rx_out) + (dy * dy) / (ry_out * ry_out);

            var rx_in = Math.max(1, rx - threshold);
            var ry_in = Math.max(1, ry - threshold);
            var dist_in = (dx * dx) / (rx_in * rx_in) + (dy * dy) / (ry_in * ry_in);

            return dist_out <= 1.0 && dist_in >= 1.0;
        }
    }

    Canvas {
        id: canvas

        anchors.fill: parent

        onHeightChanged: requestPaint()
        onWidthChanged: requestPaint()

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();

            var x = Math.min(root.localP1.x, root.localP2.x);
            var y = Math.min(root.localP1.y, root.localP2.y);
            var w = Math.abs(root.localP2.x - root.localP1.x);
            var h = Math.abs(root.localP2.y - root.localP1.y);

            var cx = x + w / 2;
            var cy = y + h / 2;
            var rx = w / 2;
            var ry = h / 2;

            ctx.beginPath();

            var kappa = 0.5522848;
            var ox = rx * kappa;
            var oy = ry * kappa;
            ctx.moveTo(cx - rx, cy);
            ctx.bezierCurveTo(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
            ctx.bezierCurveTo(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
            ctx.bezierCurveTo(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
            ctx.bezierCurveTo(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);

            ctx.closePath();

            if (root.hasOutline) {
                ctx.lineWidth = root.lineWidth;
                ctx.strokeStyle = root.color;
                ctx.stroke();
            } else {
                ctx.fillStyle = root.color;
                ctx.fill();

                if (root.hasStroke) {
                    ctx.lineWidth = 2;
                    ctx.strokeStyle = "white";
                    ctx.stroke();
                }
            }
        }

        Connections {
            function onColorChanged() {
                canvas.requestPaint();
            }
            function onHasOutlineChanged() {
                canvas.requestPaint();
            }
            function onHasStrokeChanged() {
                canvas.requestPaint();
            }
            function onLineWidthChanged() {
                canvas.requestPaint();
            }
            function onP1Changed() {
                canvas.requestPaint();
            }
            function onP2Changed() {
                canvas.requestPaint();
            }
            target: root
        }
    }
}
