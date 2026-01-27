#pragma once
#include <QtQuick/QQuickImageProvider>
#include <QImage>
#include <QQmlApplicationEngine>
#include <mutex>

QImage get_capture_qimage(QString id) noexcept;

class MinnowImageProvider : public QQuickImageProvider {
    QString m_lastId;
    QImage m_cachedImage;
    std::mutex m_mutex;

public:
    MinnowImageProvider() : QQuickImageProvider(QQuickImageProvider::Image) {}
    
    QImage requestImage(const QString &id, QSize *size, const QSize &requestedSize) override {
        QImage img;
        bool found = false;

        {
            std::lock_guard<std::mutex> lock(m_mutex);
            if (id == m_lastId && !m_cachedImage.isNull()) {
                img = m_cachedImage;
                found = true;
            }
        }

        if (!found) {
            img = get_capture_qimage(id);
            {
                std::lock_guard<std::mutex> lock(m_mutex);
                m_lastId = id;
                m_cachedImage = img;
            }
        }
        
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
