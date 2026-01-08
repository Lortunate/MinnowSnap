#pragma once
#include <QtQuick/QQuickImageProvider>
#include <QImage>
#include <QQmlApplicationEngine>

QImage get_capture_qimage(QString id) noexcept;

class MinnowImageProvider : public QQuickImageProvider {
public:
    MinnowImageProvider() : QQuickImageProvider(QQuickImageProvider::Image) {}
    
    QImage requestImage(const QString &id, QSize *size, const QSize &requestedSize) override {
        QImage img = get_capture_qimage(id);
        
        if (size) *size = img.size();
        
        if (requestedSize.width() > 0 && requestedSize.height() > 0) {
             return img.scaled(requestedSize, Qt::KeepAspectRatio, Qt::SmoothTransformation);
        }
        if (requestedSize.width() > 0) {
            return img.scaledToWidth(requestedSize.width(), Qt::SmoothTransformation);
        }
        if (requestedSize.height() > 0) {
            return img.scaledToHeight(requestedSize.height(), Qt::SmoothTransformation);
        }
        return img;
    }
};

inline void register_provider(QQmlApplicationEngine& engine) {
    engine.addImageProvider("minnow", new MinnowImageProvider());
}

inline QImage create_from_rgba(const unsigned char* data, int width, int height) {
    return QImage(data, width, height, QImage::Format_RGBA8888).copy();
}