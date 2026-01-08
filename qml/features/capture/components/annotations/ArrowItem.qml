import QtQuick
import com.lortunate.minnow

AnnotationBase {
    id: root

    readonly property string type: "arrow"

    // Config
    padding: Math.max(12, lineWidth * 3)
    maintainAspectRatio: false
    resizable: true

    // Calculate bounding box based on p1, p2 and padding
    x: Math.min(p1.x, p2.x) - padding
    y: Math.min(p1.y, p2.y) - padding
    width: Math.abs(p1.x - p2.x) + (padding * 2)
    height: Math.abs(p1.y - p2.y) + (padding * 2)

    function intersect(x1, y1, x2, y2, x3, y3, x4, y4) {
        var denom = (y4 - y3) * (x2 - x1) - (x4 - x3) * (y2 - y1);
        if (denom === 0)
            return null;
        var ua = ((x4 - x3) * (y1 - y3) - (y4 - y3) * (x1 - x3)) / denom;
        return {
            x: x1 + ua * (x2 - x1),
            y: y1 + ua * (y2 - y1)
        };
    }

    // Override isHit
    function isHit(mx, my) {
        var threshold = 10; // Hit radius
        var p1 = root.localP1;
        var p2 = root.localP2;

        var vx = p2.x - p1.x;
        var vy = p2.y - p1.y;
        var lenSq = vx * vx + vy * vy;

        var wx = mx - p1.x;
        var wy = my - p1.y;

        var t = 0;
        if (lenSq > 0) {
            t = (wx * vx + wy * vy) / lenSq;
            t = Math.max(0, Math.min(1, t));
        }

        var closestX = p1.x + t * vx;
        var closestY = p1.y + t * vy;

        var distSq = (mx - closestX) * (mx - closestX) + (my - closestY) * (my - closestY);
        return distSq <= threshold * threshold;
    }

    Canvas {
        id: canvas

        anchors.fill: parent

        onHeightChanged: requestPaint()
        onWidthChanged: requestPaint()

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();

            // Calculation
            var headLength = 12 + root.lineWidth * 3;
            var arrowAngle = Math.PI / 7;
            var indentRatio = 0.2;

            var dx = root.localP2.x - root.localP1.x;
            var dy = root.localP2.y - root.localP1.y;
            var angle = Math.atan2(dy, dx);

            var indentDist = headLength * (1.0 - indentRatio);
            var innerX = root.localP2.x - indentDist * Math.cos(angle);
            var innerY = root.localP2.y - indentDist * Math.sin(angle);

            var len = Math.sqrt(dx * dx + dy * dy);
            var nx = 0;
            var ny = 0;
            if (len > 0) {
                nx = -dy / len;
                ny = dx / len;
            }

            var halfWidth = root.lineWidth / 2.0;
            var tailRadius = Math.max(0.5, halfWidth * 0.1);

            var b1x = innerX + nx * halfWidth;
            var b1y = innerY + ny * halfWidth;
            var b2x = innerX - nx * halfWidth;
            var b2y = innerY - ny * halfWidth;

            var t1x = root.localP1.x + nx * tailRadius;
            var t1y = root.localP1.y + ny * tailRadius;
            var t2x = root.localP1.x - nx * tailRadius;
            var t2y = root.localP1.y - ny * tailRadius;

            var w1x = root.localP2.x - headLength * Math.cos(angle - arrowAngle);
            var w1y = root.localP2.y - headLength * Math.sin(angle - arrowAngle);
            var w2x = root.localP2.x - headLength * Math.cos(angle + arrowAngle);
            var w2y = root.localP2.y - headLength * Math.sin(angle + arrowAngle);

            var i1 = root.intersect(t1x, t1y, b1x, b1y, innerX, innerY, w1x, w1y);
            var i2 = root.intersect(t2x, t2y, b2x, b2y, innerX, innerY, w2x, w2y);

            if (!i1)
                i1 = {
                    x: b1x,
                    y: b1y
                };
            if (!i2)
                i2 = {
                    x: b2x,
                    y: b2y
                };

            ctx.beginPath();
            ctx.moveTo(t1x, t1y);
            ctx.lineTo(i1.x, i1.y);
            ctx.lineTo(w1x, w1y);
            ctx.lineTo(root.localP2.x, root.localP2.y);
            ctx.lineTo(w2x, w2y);
            ctx.lineTo(i2.x, i2.y);
            ctx.lineTo(t2x, t2y);
            ctx.arc(root.localP1.x, root.localP1.y, tailRadius, angle + Math.PI / 2, angle - Math.PI / 2);
            ctx.closePath();

            if (root.hasOutline) {
                // Outline mode: Always just a colored stroke (hollow)
                ctx.lineWidth = 2;
                ctx.strokeStyle = root.color;
                ctx.stroke();
            } else {
                // Solid mode: Filled
                ctx.fillStyle = root.color;
                ctx.fill();

                // Optional Border for Solid mode
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
