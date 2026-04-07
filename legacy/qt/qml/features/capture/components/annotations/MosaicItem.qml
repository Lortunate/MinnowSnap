import QtQuick
import QtQuick.Effects
import com.lortunate.minnow

AnnotationBase {
    id: root

    required property Item sourceItem
    required property var targetLayer

    readonly property string type: "mosaic"
    property string mosaicType: "mosaic"
    property real intensity: 10

    hasOutline: false
    maintainAspectRatio: false
    padding: 0
    resizable: true

    height: Math.abs(p1.y - p2.y)
    width: Math.abs(p1.x - p2.x)
    x: Math.min(p1.x, p2.x)
    y: Math.min(p1.y, p2.y)

    function handleResize(delta) {
        var step = 2;
        root.intensity = Math.max(2, Math.min(64, root.intensity + (delta * step)));
    }

    function isHit(mx, my) {
        if (root.drawingMode) {
            return false;
        }
        return mx >= 0 && mx <= width && my >= 0 && my <= height;
    }

    ShaderEffectSource {
        id: captureSource

        anchors.fill: parent
        live: true
        hideSource: false
        visible: root.width > 5 && root.height > 5 && root.mosaicType === "mosaic"

        sourceItem: root.sourceItem
        sourceRect: Qt.rect(Math.round(root.targetLayer.layerX + root.x), Math.round(root.targetLayer.layerY + root.y), Math.max(1, root.width), Math.max(1, root.height))

        textureSize: {
            if (root.mosaicType === "mosaic") {
                var w = Math.max(1, Math.ceil(root.width / Math.max(2, root.intensity)));
                var h = Math.max(1, Math.ceil(root.height / Math.max(2, root.intensity)));
                return Qt.size(w, h);
            }
            return Qt.size(Math.max(1, root.width), Math.max(1, root.height));
        }

        smooth: root.mosaicType !== "mosaic"
    }

    MultiEffect {
        id: blurDisplay

        anchors.fill: parent
        source: captureSource
        visible: root.width > 5 && root.height > 5 && root.mosaicType === "blur"

        blurEnabled: true
        blur: Math.min(1.0, Math.max(0.0, root.intensity / 64.0))
        blurMax: 128
        autoPaddingEnabled: false
    }

    Rectangle {
        anchors.fill: parent
        border.color: AppTheme.primary
        border.width: 1
        color: "transparent"
        visible: root.selected

        Rectangle {
            anchors.fill: parent
            anchors.margins: 1
            border.color: "white"
            border.width: 1
            color: "transparent"
            opacity: 0.5
        }
    }
}
