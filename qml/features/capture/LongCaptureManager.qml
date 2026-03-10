import QtQuick
import QtCore
import com.lortunate.minnow

Item {
    id: root

    property string pendingScrollAction: ""

    property var screenCapture

    property bool scrollCancelled: false
    property var pinnedWindows: []

    signal requestHide
    signal requestReset
    signal requestShow

    function finish(retry) {
        scrollToolbar.isBusy = false;
        scrollToolbar.hide();
        scrollFrame.hide();
        scrollPreview.hide();
        root.requestReset();

        if (retry && !scrollCancelled) {
            root.requestShow(); // Retry
        }
        scrollCancelled = false;
    }
    function processScrollResult(path) {
        if (!scrollCancelled && path !== "") {
            screenCapture.requestAction(path, pendingScrollAction, 0, 0, 0, 0, false);
            finish(false);
        } else {
            finish(true);
        }
    }
    function start(x, y, w, h) {
        scrollFrame.selectionX = x;
        scrollFrame.selectionY = y;
        scrollFrame.selectionWidth = w;
        scrollFrame.selectionHeight = h;
        scrollFrame.show();

        scrollToolbar.selectionX = x;
        scrollToolbar.selectionY = y;
        scrollToolbar.selectionWidth = w;
        scrollToolbar.selectionHeight = h;
        scrollToolbar.show();

        scrollPreview.selectionX = x;
        scrollPreview.selectionY = y;
        scrollPreview.selectionWidth = w;
        scrollPreview.selectionHeight = h;
        scrollPreview.show();

        root.requestHide();
    }
    function stop() {
        screenCapture.stopScrollCapture();
    }

    // Windows
    resources: [
        LongCaptureFrame {
            id: scrollFrame

            visible: false
        },
        LongCaptureToolbarWindow {
            id: scrollToolbar

            visible: false

            onActionClicked: action => {
                if (action === "cancel") {
                    scrollCancelled = true;
                    scrollToolbar.visible = false;
                    scrollFrame.visible = false;
                    scrollPreview.visible = false;
                    root.stop();
                } else {
                    pendingScrollAction = action;
                    // Enter busy state explicitly using ID to avoid scope issues
                    scrollToolbar.isBusy = true;
                    scrollToolbar.busyText = qsTr("Processing...");
                    scrollFrame.visible = false; // Hide frame to indicate capture stopped
                    // Keep toolbar visible to show progress
                    root.stop();
                }
            }
        },
        LongCapturePreviewWindow {
            id: scrollPreview

            visible: false
        }
    ]

    Connections {

        function onScrollCaptureStarted(x, y, w, h) {
            start(x, y, w, h);
        }

        function onCaptureReady() {
            if (scrollToolbar.visible)
                finish(false);
        }

        function onScrollCaptureFinished(path) {
            processScrollResult(path);
        }
        function onScrollCaptureUpdated(height) {
            scrollPreview.refresh(height);
            scrollFrame.flashSuccess();
            scrollFrame.warningText = "";
        }
        function onScrollCaptureWarning(msg) {
            scrollFrame.warningText = msg;
        }

        target: screenCapture
    }
}
