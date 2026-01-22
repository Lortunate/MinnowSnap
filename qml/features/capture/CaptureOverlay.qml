import QtQuick
import QtQuick.Window
import com.lortunate.minnow
import "components"
import "components/annotations"
import "../../components"

Window {
    id: overlayWindow

    property string backgroundImageSource: ""
    property alias currentSelection: controller.selectionRect
    readonly property rect activeRect: {
        if (isLockedState || controller.state === states.dragging) {
            return controller.selectionRect;
        } else if (isBrowsingWithTarget) {
            return controller.targetRect;
        }
        return Qt.rect(0, 0, 0, 0);
    }
    readonly property bool hasSelection: activeRect.width > 0 && activeRect.height > 0

    readonly property bool isLockedState: controller.state === states.locked || controller.state === states.moving || controller.state === states.resizing
    readonly property bool isBrowsingWithTarget: controller.state === states.browsing && controller.hasTarget

    readonly property color maskColor: AppTheme.overlayMask
    property bool processing: false
    property var screenCapture: null
    readonly property color selectionColor: AppTheme.selection
    readonly property color selectionFillColor: AppTheme.selectionFill

    signal cancelled
    signal selectionMade(int x, int y, int width, int height)

    function cancelCapture() {
        if (processing)
            return;
        overlayWindow.hide();
        resetState();
        cancelled();
    }
    function confirmSelection(action) {
        if (processing)
            return;

        if (action === "undo") {
            annotationLayer.undo();
            return;
        }
        if (action === "redo") {
            annotationLayer.redo();
            return;
        }

        if (action === "scroll") {
            longCaptureManager.start(currentSelection.x, currentSelection.y, currentSelection.width, currentSelection.height);
            return;
        }

        // Delegate to Compositor
        captureCompositor.capture(currentSelection, action);
    }
    function endResize() {
        controller.endResize();
    }

    function resetState() {
        processing = false;
        controller.reset();
        annotationLayer.clear();
        screenCapture.isCapturing = false;
    }

    function constrainToolbarPos(targetX, targetY, targetW, targetH, itemW, itemH, isAbove) {
        // Center X relative to target, clamped to screen bounds with padding
        let desiredX = targetX + targetW - itemW;
        let x = Math.max(10, Math.min(overlayWindow.width - itemW - 10, desiredX));

        // Y positioning
        let y;
        if (isAbove) {
            let aboveY = targetY - itemH - 8;
            // If not enough space above, try below, else force to top edge
            y = (aboveY >= 0) ? aboveY : (targetY + targetH + 8);
        } else {
            let belowY = targetY + targetH + 8;
            let aboveY = targetY - itemH - 8;
            // Prefer below, if not enough space, go above, else force to padding
            y = (belowY + itemH <= overlayWindow.height) ? belowY : (aboveY >= 0 ? aboveY : 40);
        }

        return Qt.point(x, y);
    }

    function startResize(corner, mouseX, mouseY) {
        controller.startResize(corner, mouseX, mouseY);
    }
    function updateResize(mouseX, mouseY) {
        controller.updateResize(mouseX, mouseY);
    }

    color: "transparent"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool
    height: Screen.height
    visible: false
    width: Screen.width
    x: Screen.virtualX
    y: Screen.virtualY

    Component.onCompleted: {
        controller.setupWindow(overlayWindow);
    }

    onVisibleChanged: {
        if (visible) {
            focusScope.forceActiveFocus();
        }
    }

    OverlayController {
        id: controller

        screenHeight: overlayWindow.height
        screenWidth: overlayWindow.width
    }

    QtObject {
        id: states

        readonly property string browsing: "BROWSING"
        readonly property string dragging: "DRAGGING"
        readonly property string locked: "LOCKED"
        readonly property string moving: "MOVING"
        readonly property string resizing: "RESIZING"
    }

    Connections {
        function onScreenshotCaptured(path) {
            if (path !== "") {
                overlayWindow.backgroundImageSource = path + "?t=" + Date.now();
                overlayWindow.show();
                overlayWindow.raise();
                overlayWindow.requestActivate();
                resetState();
            }
        }
        function onWindowInfoReady(json) {
            controller.updateWindowList(json);
        }
        function onCaptureReady() {
            overlayWindow.backgroundImageSource = "image://minnow/preview?t=" + Date.now();
            overlayWindow.show();
            overlayWindow.raise();
            overlayWindow.requestActivate();
            resetState();
        }

        target: screenCapture
    }

    FocusScope {
        id: focusScope

        anchors.fill: parent
        focus: true

        Keys.onEnterPressed: {
            if (hasSelection && !processing)
                confirmSelection("copy");
        }
        Keys.onEscapePressed: {
            cancelCapture();
        }
        Keys.onPressed: event => {
            if (!isLockedState && controller.state !== states.dragging) {
                if (event.key === Qt.Key_C) {
                    colorPicker.copyColor();
                    event.accepted = true;
                }
                if (event.key === Qt.Key_Shift && !event.isAutoRepeat) {
                    colorPicker.cycleFormat();
                }
            }

            // Ctrl+Z / Cmd+Z to Undo
            if ((event.key === Qt.Key_Z) && (event.modifiers & Qt.ControlModifier)) {
                if (controller.state === states.locked) {
                    if (event.modifiers & Qt.ShiftModifier) {
                        annotationLayer.redo();
                    } else {
                        annotationLayer.undo();
                    }
                    event.accepted = true;
                }
            }
        }
        Keys.onReturnPressed: {
            if (hasSelection && !processing)
                confirmSelection("copy");
        }

        Image {
            id: bgImage

            anchors.fill: parent
            fillMode: Image.PreserveAspectCrop
            horizontalAlignment: Image.AlignLeft
            source: overlayWindow.backgroundImageSource
            verticalAlignment: Image.AlignTop
            layer.enabled: true
        }
        SelectionMask {
            activeH: activeRect.height
            activeW: activeRect.width
            activeX: activeRect.x
            activeY: activeRect.y
            anchors.fill: parent
            maskColor: overlayWindow.maskColor
            visible: hasSelection
        }
        Rectangle {
            anchors.fill: parent
            color: overlayWindow.maskColor
            visible: !hasSelection
        }

        MouseArea {
            id: mouseArea

            acceptedButtons: Qt.LeftButton | Qt.RightButton
            anchors.fill: parent
            cursorShape: {
                if (controller.state === states.locked) {
                    if (containsMouseInRect(mouseX, mouseY)) {
                        return Qt.SizeAllCursor;
                    }
                    return Qt.ArrowCursor;
                }
                return (controller.state === states.browsing && controller.hasTarget) ? Qt.PointingHandCursor : Qt.CrossCursor;
            }
            hoverEnabled: true

            function containsMouseInRect(mx, my) {
                return mx >= currentSelection.x && mx <= currentSelection.x + currentSelection.width && my >= currentSelection.y && my <= currentSelection.y + currentSelection.height;
            }

            onPositionChanged: mouse => {
                if (controller.state === states.browsing) {
                    controller.updateHover(mouse.x, mouse.y);
                } else if (controller.state === states.dragging) {
                    controller.updateSelection(mouse.x, mouse.y);
                } else if (controller.state === states.moving) {
                    controller.updateMove(mouse.x, mouse.y);
                }
            }
            onPressed: mouse => {
                if (mouse.button === Qt.RightButton) {
                    if (controller.state === states.locked) {
                        controller.reset(); // Go back to browsing
                    } else {
                        cancelCapture();
                    }
                    return;
                }

                if (controller.state === states.locked) {
                    annotationLayer.deselectAll();
                    // Check if clicking inside selection to move it
                    if (containsMouseInRect(mouse.x, mouse.y)) {
                        controller.startMove(mouse.x, mouse.y);
                    }
                } else if (controller.state === states.browsing) {
                    controller.startSelection(mouse.x, mouse.y);
                }
            }
            onReleased: mouse => {
                if (controller.state === states.dragging) {
                    controller.endSelection();
                } else if (controller.state === states.moving) {
                    controller.endMove();
                }
            }
        }

        Item {
            id: container

            anchors.fill: parent

            SelectionRect {
                id: unifiedSelectionRect

                // If locked, we bind strictly to the rectProperty (and enable handles).
                // If not locked (browsing/dragging), we manually update x/y/w/h via direct binding to activeRect.
                bindToRect: isLockedState

                // When not bound, follow activeRect
                x: !bindToRect ? activeRect.x : 0
                y: !bindToRect ? activeRect.y : 0
                width: !bindToRect ? activeRect.width : 0
                height: !bindToRect ? activeRect.height : 0

                rectProperty: overlayWindow.currentSelection // Used when bindToRect is true
                overlayWindow: overlayWindow
                visible: hasSelection || isBrowsingWithTarget

                AnnotationLayer {
                    id: annotationLayer

                    activeTool: toolbar.activeTool
                    anchors.fill: parent
                    clip: true
                    sourceImage: bgImage
                    layerX: unifiedSelectionRect.x
                    layerY: unifiedSelectionRect.y
                    visible: isLockedState
                    onRequestSetTool: tool => toolbar.activeTool = tool
                }
            }

            InfoTooltip {
                id: infoBox
                text: controller.targetInfo
                visible: isBrowsingWithTarget
                x: activeRect.x
                y: {
                    let val = activeRect.y - height - 8;
                    return val < 0 ? activeRect.y + activeRect.height + 8 : val;
                }
            }

            ResolutionTooltip {
                id: resolutionBox
                property bool showAbove: activeRect.y - height - 8 >= 0

                heightValue: activeRect.height
                visible: isLockedState || controller.state === states.dragging
                widthValue: activeRect.width
                x: activeRect.x
                y: showAbove ? activeRect.y - height - 8 : activeRect.y + activeRect.height + 8
            }

            BusyStatus {
                anchors.centerIn: parent
                running: processing
            }

            ColorPicker {
                id: colorPicker
                imageSource: overlayWindow.backgroundImageSource
                mouseX: mouseArea.mouseX
                mouseY: mouseArea.mouseY
                screenCapture: overlayWindow.screenCapture
                surfaceWidth: overlayWindow.width
                surfaceHeight: overlayWindow.height
                visible: !isLockedState && controller.state !== states.dragging
                onColorCopied: cancelCapture()
            }
        }

        SelectionToolbar {
            id: toolbar

            property point pos: constrainToolbarPos(currentSelection.x, currentSelection.y, currentSelection.width, currentSelection.height, width, height, false)

            visible: controller.state === states.locked
            x: pos.x
            y: pos.y

            onActionConfirmed: action => overlayWindow.confirmSelection(action)
            onCanceled: {
                cancelCapture();
            }
        }
        AnnotationProperties {
            id: propBar

            // For props, we target the Toolbar
            property point pos: constrainToolbarPos(toolbar.x, toolbar.y, toolbar.width, toolbar.height, width, height, false)

            activeColor: annotationLayer.activeColor
            hasOutline: annotationLayer.activeHasOutline
            hasStroke: annotationLayer.activeHasStroke
            mosaicType: annotationLayer.activeMosaicType
            mode: {
                if (annotationLayer.selectedItem) {
                    return annotationLayer.selectedItem.type;
                }
                if (toolbar.activeTool === "arrow")
                    return "arrow";
                if (toolbar.activeTool === "rectangle")
                    return "rectangle";
                if (toolbar.activeTool === "circle")
                    return "circle";
                if (toolbar.activeTool === "counter")
                    return "counter";
                if (toolbar.activeTool === "text")
                    return "text";
                if (toolbar.activeTool === "mosaic")
                    return "mosaic";
                return "arrow";
            }
            activeSize: {
                if (mode === "counter")
                    return annotationLayer.activeCounterSize;
                if (mode === "text")
                    return annotationLayer.activeFontSize;
                if (mode === "mosaic")
                    return annotationLayer.activeIntensity;
                return annotationLayer.activeLineWidth;
            }
            visible: controller.state === states.locked && (toolbar.activeTool === "arrow" || toolbar.activeTool === "rectangle" || toolbar.activeTool === "circle" || toolbar.activeTool === "counter" || toolbar.activeTool === "text" || toolbar.activeTool === "mosaic" || annotationLayer.selectedItem)

            x: pos.x
            y: pos.y

            onRequestColorChange: c => annotationLayer.activeColor = c
            onRequestOutlineChange: enabled => annotationLayer.activeHasOutline = enabled
            onRequestStrokeChange: enabled => annotationLayer.activeHasStroke = enabled
            onRequestMosaicTypeChange: type => annotationLayer.activeMosaicType = type
            onRequestSizeChange: size => {
                if (mode === "counter")
                    annotationLayer.activeCounterSize = size;
                else if (mode === "text")
                    annotationLayer.activeFontSize = size;
                else if (mode === "mosaic")
                    annotationLayer.activeIntensity = size;
                else
                    annotationLayer.activeLineWidth = size;
            }
        }
    }

    LongCaptureManager {
        id: longCaptureManager

        screenCapture: overlayWindow.screenCapture

        onRequestHide: overlayWindow.hide()
        onRequestReset: resetState()
        onRequestShow: overlayWindow.show()
    }
    CaptureCompositor {
        id: captureCompositor

        annotationLayer: annotationLayer
        lockedSelectionRect: unifiedSelectionRect // Updated reference
        overlayWindow: overlayWindow
        screenCapture: overlayWindow.screenCapture

        onProcessingChanged: overlayWindow.processing = processing
        onRequestHide: overlayWindow.hide()
        onRequestResetState: resetState()
        onSelectionMade: (x, y, w, h) => overlayWindow.selectionMade(x, y, w, h)
    }
}
