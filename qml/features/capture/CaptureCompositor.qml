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

    function performComposition(selectionRect, action) {
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

            let savePath = screenCapture.generateTempPath("png");

            compositor.grabToImage(function (result) {
                if (lockedSelectionRect) {
                    annotationLayer.parent = lockedSelectionRect;
                    annotationLayer.anchors.fill = lockedSelectionRect;
                }

                if (result.saveToFile(savePath)) {
                    screenCapture.submitCapture(savePath, action, selectionRect.x, selectionRect.y, selectionRect.width, selectionRect.height);
                } else {
                    console.error("Failed to save composite image");
                    root.requestHide();
                    root.requestResetState();
                }

                compositor.width = 0;
                compositor.height = 0;

                processing = false;
            }, Qt.size(outW, outH));
            return;
        }
        // Should not happen if flow is correct, but safe fallback:
        screenCapture.submitCapture(overlayWindow.backgroundImageSource, action, selectionRect.x, selectionRect.y, selectionRect.width, selectionRect.height);
    }
    
    function showPin(path, rect, autoOcr) {
        let component = Qt.createComponent("../pin/PinWindow.qml");
        if (component.status === Component.Ready) {
            let shadowMargin = 20;
            let finalPath = PathUtils.toUrl(path);

            let win = component.createObject(null, {
                "imageSource": finalPath,
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

        visible: true
        width: 0
        height: 0

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

    Connections {
        target: screenCapture
        function onPinWindowRequested(path, x, y, w, h, autoOcr) {
            showPin(path, Qt.rect(x,y,w,h), autoOcr);
        }
    }
}
