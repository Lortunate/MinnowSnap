import QtQuick
import QtQuick.Window
import QtCore
import com.lortunate.minnow

Item {
    id: root

    property var annotationLayer
    property var lockedSelectionRect
    property var overlayWindow

    property bool processing: false
    property var pinnedWindows: []

    property var screenCapture

    signal finished
    signal requestHide
    signal requestResetState
    signal selectionMade(int x, int y, int width, int height)

    function capture(selectionRect, action) {
        if (annotationLayer && annotationLayer.hasAnnotations) {
            processing = true;

            let srcW = compositorBg.sourceSize.width;
            let logW = compositorBg.width;
            let dpr = (srcW > 0 && logW > 0) ? (srcW / logW) : Screen.devicePixelRatio;

            compositor.width = selectionRect.width;
            compositor.height = selectionRect.height;

            compositorBg.x = -selectionRect.x;
            compositorBg.y = -selectionRect.y;

            annotationLayer.parent = annotationWrapper;
            annotationLayer.anchors.fill = annotationWrapper;

            // Deselect all annotations to hide handles/anchors
            annotationLayer.deselectAll();

            let outW = Math.ceil(selectionRect.width * dpr);
            let outH = Math.ceil(selectionRect.height * dpr);

            let tempPath = screenCapture.generateTempPath("png");
            let savePath = tempPath.startsWith("file://") ? tempPath.substring(7) : tempPath;

            compositor.grabToImage(function (result) {
                if (lockedSelectionRect) {
                    annotationLayer.parent = lockedSelectionRect;
                    annotationLayer.anchors.fill = lockedSelectionRect;
                }

                if (result.saveToFile(savePath)) {
                    processResult(action, "file://" + savePath, selectionRect);
                } else {
                    console.error("Failed to save composite image");
                    root.requestHide();
                    root.requestResetState();
                }
                processing = false;
            }, Qt.size(outW, outH));
            return;
        }

        if (action === "copy") {
            screenCapture.copyImage(overlayWindow.backgroundImageSource, selectionRect.x, selectionRect.y, selectionRect.width, selectionRect.height);
        } else if (action === "save") {
            screenCapture.saveImage(overlayWindow.backgroundImageSource, selectionRect.x, selectionRect.y, selectionRect.width, selectionRect.height);
        } else if (action === "pin" || action === "ocr") {
            let path = screenCapture.cropImage(overlayWindow.backgroundImageSource, selectionRect.x, selectionRect.y, selectionRect.width, selectionRect.height);
            if (path !== "") {
                // path already contains "file://" from ScreenCapture.cropImage (via CaptureService::save_temp)
                showPin(path, selectionRect, action === "ocr");
            }
        }
        selectionMade(selectionRect.x, selectionRect.y, selectionRect.width, selectionRect.height);
        root.requestHide();
        root.requestResetState();
    }
    function processResult(action, path, selectionRect) {
        if (action === "copy") {
            screenCapture.copyImage(path, 0, 0, 0, 0);
        } else if (action === "save") {
            screenCapture.saveImage(path, 0, 0, 0, 0);
        } else if (action === "pin" || action === "ocr") {
            showPin(path, selectionRect, action === "ocr");
        }
        selectionMade(selectionRect.x, selectionRect.y, selectionRect.width, selectionRect.height);
        root.requestHide();
        root.requestResetState();
    }
    function showPin(path, rect, autoOcr) {
        let component = Qt.createComponent("../pin/PinWindow.qml");
        if (component.status === Component.Ready) {
            let shadowMargin = 20;
            let win = component.createObject(null, {
                "imageSource": path,
                "screenCapture": root.screenCapture,
                "x": rect.x - shadowMargin,
                "y": rect.y - shadowMargin,
                "width": rect.width + (shadowMargin * 2),
                "height": rect.height + (shadowMargin * 2),
                "autoOcr": autoOcr === true
            });

            // Keep reference to prevent GC
            pinnedWindows.push(win);

            win.closing.connect(function () {
                let index = pinnedWindows.indexOf(win);
                if (index > -1) {
                    pinnedWindows.splice(index, 1);
                }
            });

            win.show();
        }
    }

    // The compositor (Visual tree for grabbing)
    Item {
        id: compositor

        visible: true // Must be visible for grabToImage

        z: -100

        // Background Container (Crops the background to the selection)
        Item {
            anchors.fill: parent
            clip: true

            Image {
                id: compositorBg

                // Match the on-screen background properties exactly
                fillMode: Image.PreserveAspectCrop
                height: overlayWindow ? overlayWindow.height : 0
                horizontalAlignment: Image.AlignLeft
                source: overlayWindow ? overlayWindow.backgroundImageSource : ""
                verticalAlignment: Image.AlignTop
                width: overlayWindow ? overlayWindow.width : 0
            }
        }

        // Wrapper for annotations
        Item {
            id: annotationWrapper

            anchors.fill: parent
        }
    }
}
