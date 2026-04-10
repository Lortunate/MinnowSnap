pragma Singleton
import QtQuick

QtObject {
    function toUrl(path) {
        if (!path)
            return "";
        const p = path.toString().replace(/\\/g, "/");
        if (p.indexOf("image://") === -1 && p.indexOf("file://") === -1)
            return "file:///" + p;
        return p;
    }
}
