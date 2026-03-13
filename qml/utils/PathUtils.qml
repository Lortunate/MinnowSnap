pragma Singleton
import QtQuick

QtObject {
    function toLocalPath(path) {
        if (!path)
            return "";
        const p = path.toString();
        if (p.startsWith("file://"))
            return p.substring(7);
        return p;
    }

    function toUrl(path) {
        if (!path)
            return "";
        const p = path.toString().replace(/\\/g, "/");
        if (p.indexOf("image://") === -1 && p.indexOf("file://") === -1)
            return "file:///" + p;
        return p;
    }

    function normalize(path) {
        if (!path)
            return "";
        return path.toString().replace(/\\/g, "/");
    }

}
