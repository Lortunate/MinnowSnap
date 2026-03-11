import QtQuick
import QtCore
import com.lortunate.minnow

Item {
    id: root

    property var screenCapture

    signal requestHide
    signal requestReset
    signal requestShow

    function finish() {
        scrollToolbar.isBusy = false;
        scrollToolbar.hide()
        scrollFrame.hide()
        scrollPreview.hide()
        root.requestReset()
    }

    function start(x, y, w, h) {
        scrollFrame.selectionX = x
        scrollFrame.selectionY = y
        scrollFrame.selectionWidth = w
        scrollFrame.selectionHeight = h
        scrollFrame.show()

        scrollToolbar.selectionX = x
        scrollToolbar.selectionY = y
        scrollToolbar.selectionWidth = w
        scrollToolbar.selectionHeight = h
        scrollToolbar.show()

        scrollPreview.selectionX = x
        scrollPreview.selectionY = y
        scrollPreview.selectionWidth = w
        scrollPreview.selectionHeight = h
        scrollPreview.show()

        root.requestHide()
    }

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
                    scrollToolbar.visible = false
                    scrollFrame.visible = false
                    scrollPreview.visible = false
                    screenCapture.cancelScrollCapture()
                } else {
                    scrollToolbar.isBusy = true
                    scrollToolbar.busyText = qsTr("Processing...")
                    scrollFrame.visible = false 
                    screenCapture.requestScrollAction(action)
                }
            }
        },
        LongCapturePreviewWindow {
            id: scrollPreview
            visible: false
        }
    ]

    Connections {
        target: screenCapture

        function onScrollCaptureStarted(x, y, w, h) {
            start(x, y, w, h)
        }

        function onCaptureReady() {
            if (scrollToolbar.visible)
                finish()
        }

        function onScrollCaptureFinished(path) {
            finish()
        }

        function onScrollCaptureUpdated(height) {
            scrollPreview.refresh(height)
            scrollFrame.flashSuccess()
            scrollFrame.warningText = ""
        }

        function onScrollCaptureWarning(msg) {
            scrollFrame.warningText = msg
        }
    }
}
