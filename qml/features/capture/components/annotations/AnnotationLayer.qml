import QtQuick
import com.lortunate.minnow

Item {
    id: root

    property color activeColor: AppTheme.danger
    property bool activeHasOutline: true
    property bool activeHasStroke: false
    property int activeLineWidth: 4
    property int activeIntensity: 10
    property string activeMosaicType: "mosaic"
    property int activeCounterSize: 32
    property int activeFontSize: 24
    property string activeTool: ""
    property int nextCounterValue: 1
    readonly property bool hasAnnotations: annotations.children.length > 0
    property var selectedItem: null
    property Item sourceImage: null

    // Global position of the layer (e.g. from parent SelectionRect)
    property real layerX: 0
    property real layerY: 0

    signal requestSetTool(string tool)

    function clear() {
        for (var i = annotations.children.length - 1; i >= 0; i--) {
            annotations.children[i].destroy();
        }
        selectedItem = null;
        nextCounterValue = 1;
    }

    function clearRedoHistory() {
        for (var i = annotations.children.length - 1; i >= 0; i--) {
            var child = annotations.children[i];
            if (!child.visible) {
                child.destroy();
            } else {
                break;
            }
        }
    }

    function createAnnotationItem(componentName, props) {
        var component = Qt.createComponent(componentName);
        if (component.status === Component.Ready) {
            var item = component.createObject(annotations, props);

            item.requestRemove.connect(function () {
                item.destroy();
                // If the destroyed item was selected, clear selection reference
                if (root.selectedItem === item) {
                    root.selectedItem = null;
                }
            });
            item.requestSelect.connect(function () {
                if (root.activeTool !== "") {
                    root.requestSetTool("");
                }
                root.deselectAll();
                item.selected = true;
                root.selectedItem = item;
            });

            return item;
        } else {
            console.error("Error creating " + componentName + ":", component.errorString());
            return null;
        }
    }

    function deselectAll() {
        for (var i = 0; i < annotations.children.length; i++) {
            if (annotations.children[i].selected !== undefined) {
                annotations.children[i].selected = false;
            }
        }
        selectedItem = null;
    }
    function redo() {
        for (var i = 0; i < annotations.children.length; i++) {
            var item = annotations.children[i];
            if (!item.visible) {
                item.visible = true;
                return;
            }
        }
    }
    function undo() {
        for (var i = annotations.children.length - 1; i >= 0; i--) {
            var item = annotations.children[i];
            if (item.visible) {
                item.visible = false;
                if (selectedItem === item) {
                    selectedItem = null;
                }
                return;
            }
        }
    }

    onActiveToolChanged: {
        if (activeTool !== "") {
            deselectAll();
            if (activeTool === "counter" || activeTool === "arrow") {
                activeHasOutline = false;
            }
        }
    }
    onActiveColorChanged: {
        if (selectedItem && selectedItem.color !== activeColor) {
            selectedItem.color = activeColor;
        }
    }
    onActiveHasOutlineChanged: {
        if (selectedItem && selectedItem.hasOutline !== activeHasOutline) {
            selectedItem.hasOutline = activeHasOutline;
        }
    }
    onActiveHasStrokeChanged: {
        if (selectedItem && selectedItem.hasStroke !== activeHasStroke) {
            selectedItem.hasStroke = activeHasStroke;
        }
    }
    onSelectedItemChanged: {
        if (selectedItem) {
            if (activeColor !== selectedItem.color) {
                activeColor = selectedItem.color;
            }
            if (selectedItem.hasOutline !== undefined && activeHasOutline !== selectedItem.hasOutline) {
                activeHasOutline = selectedItem.hasOutline;
            }
            if (selectedItem.hasStroke !== undefined && activeHasStroke !== selectedItem.hasStroke) {
                activeHasStroke = selectedItem.hasStroke;
            }

            // Sync Size
            if (selectedItem.type === "counter") {
                activeCounterSize = selectedItem.size;
            } else if (selectedItem.type === "text") {
                activeFontSize = selectedItem.fontSize;
            } else if (selectedItem.type === "mosaic") {
                activeIntensity = selectedItem.intensity;
                activeMosaicType = selectedItem.mosaicType;
            } else if (selectedItem.lineWidth !== undefined) {
                activeLineWidth = selectedItem.lineWidth;
            }

            // Bring to front
            for (var i = 0; i < annotations.children.length; i++) {
                annotations.children[i].z = 0;
            }
            selectedItem.z = 1;
        }
    }

    onActiveLineWidthChanged: {
        if (selectedItem && selectedItem.type !== "counter" && selectedItem.type !== "text" && selectedItem.type !== "mosaic") {
            selectedItem.lineWidth = activeLineWidth;
        }
    }
    onActiveIntensityChanged: {
        if (selectedItem && selectedItem.type === "mosaic") {
            selectedItem.intensity = activeIntensity;
        }
    }
    onActiveMosaicTypeChanged: {
        if (selectedItem && selectedItem.type === "mosaic") {
            selectedItem.mosaicType = activeMosaicType;
        }
    }
    onActiveCounterSizeChanged: {
        if (selectedItem && selectedItem.type === "counter") {
            selectedItem.size = activeCounterSize;
        }
    }
    onActiveFontSizeChanged: {
        if (selectedItem && selectedItem.type === "text") {
            selectedItem.fontSize = activeFontSize;
        }
    }

    // Input area for creating new annotations (Background)
    MouseArea {
        property var currentItem: null
        property point startPoint: Qt.point(0, 0)

        anchors.fill: parent
        cursorShape: root.activeTool === "text" ? Qt.IBeamCursor : Qt.CrossCursor
        // Only enabled when a drawing tool is active.
        // When disabled (Selection Mode), clicks pass through to the underlying CaptureOverlay handles/movers.
        enabled: root.activeTool !== ""

        onPositionChanged: mouse => {
            if (currentItem) {
                var p2 = Qt.point(mouse.x, mouse.y);

                // Shift key constraint for Rectangle and Circle (Square and Standard Circle)
                if ((mouse.modifiers & Qt.ShiftModifier) && (root.activeTool === "circle" || root.activeTool === "rectangle")) {
                    var dx = p2.x - currentItem.p1.x;
                    var dy = p2.y - currentItem.p1.y;
                    var size = Math.max(Math.abs(dx), Math.abs(dy));

                    // Determine direction
                    var sx = dx >= 0 ? 1 : -1;
                    var sy = dy >= 0 ? 1 : -1;

                    p2 = Qt.point(currentItem.p1.x + sx * size, currentItem.p1.y + sy * size);
                }

                if (root.activeTool !== "text") {
                    currentItem.p2 = p2;
                }
            }
        }
        onPressed: mouse => {
            startPoint = Qt.point(mouse.x, mouse.y);

            root.deselectAll();
            root.clearRedoHistory();

            var props = {
                "color": root.activeColor,
                "selected": false,
                "p1": startPoint,
                "hasOutline": root.activeHasOutline,
                "hasStroke": root.activeHasStroke,
                "lineWidth": root.activeLineWidth
            };

            var componentMap = {
                "arrow": "ArrowItem.qml",
                "rectangle": "RectangleItem.qml",
                "circle": "CircleItem.qml",
                "counter": "CounterItem.qml",
                "text": "TextItem.qml",
                "mosaic": "MosaicItem.qml"
            };

            var componentName = componentMap[root.activeTool];
            if (!componentName)
                return;

            if (root.activeTool === "counter") {
                props["selected"] = true;
                props["number"] = root.nextCounterValue;
                props["size"] = root.activeCounterSize;
            } else if (root.activeTool === "text") {
                props["selected"] = true;
                props["fontSize"] = root.activeFontSize;
            } else if (root.activeTool === "mosaic") {
                props["p2"] = startPoint;
                props["sourceItem"] = root.sourceImage;
                props["intensity"] = root.activeIntensity;
                props["mosaicType"] = root.activeMosaicType;
                props["targetLayer"] = root;
            } else {
                props["p2"] = startPoint;
            }

            var item = root.createAnnotationItem(componentName, props);

            if (item) {
                if (root.activeTool === "counter") {
                    root.nextCounterValue++;
                    root.selectedItem = item;
                } else if (root.activeTool === "text") {
                    root.selectedItem = item;
                } else {
                    currentItem = item;
                }
            }
        }
        onReleased: mouse => {
            if (currentItem) {
                if (Math.abs(currentItem.p1.x - currentItem.p2.x) < 5 && Math.abs(currentItem.p1.y - currentItem.p2.y) < 5) {
                    currentItem.destroy();
                } else {
                    currentItem.selected = true;
                    root.selectedItem = currentItem;
                }
                currentItem = null;
            }
        }
    }

    // Container for annotations (Foreground)
    Item {
        id: annotations

        anchors.fill: parent
    }

    Shortcut {
        sequence: "Backspace"
        enabled: root.selectedItem !== null
        onActivated: root.selectedItem.requestRemove()
    }
}
