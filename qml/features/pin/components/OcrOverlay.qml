import QtQuick
import QtQuick.Controls
import com.lortunate.minnow

Item {
    id: root

    property Item targetImage
    property string sourcePath

    property bool hasSelection: Object.keys(selectedIndices).length > 0
    property var selectedIndices: ({})
    property Item activeTextBlock: null

    signal requestMenu(int x, int y)

    OcrViewModel {
        id: ocrModel
    }

    function recognize() {
        if (sourcePath)
            ocrModel.recognizeImage(sourcePath);
    }

    function copySelection() {
        if (activeTextBlock && activeTextBlock.selectedText.length > 0) {
            activeTextBlock.copy();
            activeTextBlock.deselect();
            return true;
        }

        if (!hasSelection)
            return false;

        var indexes = Object.keys(selectedIndices).map(Number).sort((a, b) => a - b);
        if (indexes.length === 0)
            return false;

        var result = blocks[indexes[0]].text;
        var prevBlock = blocks[indexes[0]];

        for (var k = 1; k < indexes.length; k++) {
            var currBlock = blocks[indexes[k]];
            var currText = currBlock.text;
            var prevText = prevBlock.text;

            var prevBottom = prevBlock.cy + prevBlock.height / 2.0;
            var currTop = currBlock.cy - currBlock.height / 2.0;
            var gap = currTop - prevBottom;
            var avgHeight = (prevBlock.height + currBlock.height) / 2.0;
            var isListItem = /^\s*(\d+\.|-|•|\*)\s/.test(currText);

            if (prevText.endsWith("-")) {
                result = result.slice(0, -1) + currText;
            } else if (gap > avgHeight * 0.5 || isListItem) {
                result += "\n" + currText;
            } else {
                var lastChar = prevText[prevText.length - 1];
                var firstChar = currText[0];
                var isPrevCJK = /[\u3000-\u303f\u3040-\u309f\u30a0-\u30ff\uff00-\uff9f\u4e00-\u9faf\u3400-\u4dbf]/.test(lastChar);
                var isCurrCJK = /[\u3000-\u303f\u3040-\u309f\u30a0-\u30ff\uff00-\uff9f\u4e00-\u9faf\u3400-\u4dbf]/.test(firstChar);

                if (isPrevCJK && isCurrCJK) {
                    result += currText;
                } else {
                    result += " " + currText;
                }
            }
            prevBlock = currBlock;
        }

        clipboardHelper.text = result;
        clipboardHelper.selectAll();
        clipboardHelper.copy();
        return true;
    }

    TextEdit {
        id: clipboardHelper
        visible: false
    }

    property var blocks: {
        if (ocrModel.ocrDataJson === "")
            return [];
        try {
            return JSON.parse(ocrModel.ocrDataJson);
        } catch (e) {
            return [];
        }
    }

    visible: blocks.length > 0

    property real imgPaintedW: targetImage ? targetImage.paintedWidth : 0
    property real imgPaintedH: targetImage ? targetImage.paintedHeight : 0
    property real offsetX: targetImage ? (targetImage.width - imgPaintedW) / 2 : 0
    property real offsetY: targetImage ? (targetImage.height - imgPaintedH) / 2 : 0

    MouseArea {
        id: selectionArea
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.IBeamCursor

        property point startPoint: Qt.point(0, 0)
        property rect selectionRect: Qt.rect(0, 0, 0, 0)
        property bool isSelecting: false
        property var baseSelection: ({})

        onPressed: mouse => {
            if (mouse.button === Qt.LeftButton) {
                if (root.activeTextBlock) {
                    root.activeTextBlock.deselect();
                    root.activeTextBlock = null;
                }

                if (mouse.modifiers & Qt.ShiftModifier) {
                    var base = {};
                    for (var key in root.selectedIndices) {
                        base[key] = true;
                    }
                    baseSelection = base;
                } else {
                    root.selectedIndices = {};
                    baseSelection = {};
                }

                isSelecting = true;
                startPoint = Qt.point(mouse.x, mouse.y);
                selectionRect = Qt.rect(mouse.x, mouse.y, 0, 0);
                root.forceActiveFocus();
            } else if (mouse.button === Qt.RightButton) {
                root.requestMenu(mouse.x, mouse.y);
            }
        }

        onPositionChanged: mouse => {
            if (isSelecting) {
                var x = Math.min(startPoint.x, mouse.x);
                var y = Math.min(startPoint.y, mouse.y);
                var w = Math.abs(mouse.x - startPoint.x);
                var h = Math.abs(mouse.y - startPoint.y);
                selectionRect = Qt.rect(x, y, w, h);
                updateSelection();
            }
        }

        onReleased: {
            isSelecting = false;
            selectionRect = Qt.rect(0, 0, 0, 0);
            baseSelection = {};
        }

        function updateSelection() {
            var newSel = {};
            for (var k in baseSelection) {
                newSel[k] = true;
            }

            for (var i = 0; i < blockRepeater.count; ++i) {
                var item = blockRepeater.itemAt(i);
                var center = item.mapToItem(selectionArea, item.width / 2, item.height / 2);
                if (center.x >= selectionRect.x && center.x <= selectionRect.x + selectionRect.width && center.y >= selectionRect.y && center.y <= selectionRect.y + selectionRect.height) {
                    newSel[i] = true;
                }
            }
            root.selectedIndices = newSel;
        }

        Rectangle {
            x: selectionArea.selectionRect.x
            y: selectionArea.selectionRect.y
            width: selectionArea.selectionRect.width
            height: selectionArea.selectionRect.height
            color: Qt.rgba(0.0, 0.48, 1.0, 0.1)
            border.color: Qt.rgba(0.0, 0.48, 1.0, 0.4)
            border.width: 1
            visible: selectionArea.isSelecting && width > 0 && height > 0
        }
    }

    Repeater {
        id: blockRepeater
        model: root.blocks
        delegate: Item {
            required property var modelData
            required property int index

            property real boxW: modelData.width * root.imgPaintedW
            property real boxH: modelData.height * root.imgPaintedH
            property bool isSelected: root.selectedIndices[index] === true

            x: root.offsetX + (modelData.cx * root.imgPaintedW) - (width / 2)
            y: root.offsetY + (modelData.cy * root.imgPaintedH) - (height / 2)
            width: boxW
            height: boxH
            rotation: modelData.angle
            transformOrigin: Item.Center

            Rectangle {
                anchors.fill: parent
                color: parent.isSelected ? Qt.rgba(0.0, 0.48, 1.0, 0.3) : (hoverHandler.containsMouse ? Qt.rgba(1, 1, 1, 0.2) : "transparent")
                radius: 2
            }

            TextEdit {
                id: textEdit
                anchors.centerIn: parent
                text: modelData.text

                readOnly: true
                selectByMouse: true
                color: "transparent"
                selectionColor: Qt.rgba(0.0, 0.48, 1.0, 0.4)
                selectedTextColor: "transparent"

                font.pixelSize: parent.height * 0.8
                padding: 0
                wrapMode: Text.NoWrap

                scale: {
                    var maxW = parent.width * 0.95;
                    if (contentWidth > maxW)
                        return maxW / contentWidth;
                    return 1.0;
                }
                transformOrigin: Item.Center

                onActiveFocusChanged: {
                    if (activeFocus) {
                        root.activeTextBlock = textEdit;
                        root.selectedIndices = {};
                    }
                }

                MouseArea {
                    id: hoverHandler
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.RightButton
                    cursorShape: Qt.IBeamCursor
                    onClicked: mouse => {
                        if (!parent.isSelected && !textEdit.activeFocus) {
                            var sel = {};
                            sel[index] = true;
                            root.selectedIndices = sel;
                        }

                        var p = mapToItem(root, mouse.x, mouse.y);
                        root.requestMenu(p.x, p.y);
                    }
                }
            }
        }
    }
}
