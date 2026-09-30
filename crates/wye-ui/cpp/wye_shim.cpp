#include "wye-ui/cpp/wye_shim.h"

#include <string>
#include <vector>

#include <QtCore/QFuture>
#include <QtCore/QMetaObject>
#include <QtCore/QPointer>
#include <QtGui/QPainterPath>
#include <QtGui/QPolygon>
#include <QtGui/QRegion>
#include <QtQuickControls2/QQuickStyle>

#include <KWaylandExtras>
#include <KWindowEffects>
#include <KWindowSystem>

namespace wye {

namespace {

// QApplication keeps references to argc and argv for its whole life, so
// they live as long as the process.
struct Arguments {
    std::vector<std::string> storage;
    std::vector<char *> pointers;
    int count = 0;
};

Arguments &arguments()
{
    static Arguments args;
    return args;
}

const char *const DESKTOP_STYLE = "org.kde.desktop";
const char *const TOKEN_SIGNAL = "activationTokenReady";

} // namespace

std::unique_ptr<QApplication> applicationNew(rust::Slice<const rust::String> args,
                                             const QString &desktopFileName,
                                             const QString &displayName)
{
    Arguments &stored = arguments();
    stored.storage.clear();
    for (const rust::String &arg : args) {
        stored.storage.emplace_back(arg.data(), arg.size());
    }
    stored.pointers.clear();
    for (std::string &arg : stored.storage) {
        stored.pointers.push_back(arg.data());
    }
    stored.pointers.push_back(nullptr);
    stored.count = static_cast<int>(stored.storage.size());

    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) {
        QQuickStyle::setStyle(QString::fromLatin1(DESKTOP_STYLE));
    }
    auto app = std::make_unique<QApplication>(stored.count, stored.pointers.data());
    QGuiApplication::setDesktopFileName(desktopFileName);
    QGuiApplication::setApplicationDisplayName(displayName);
    // Resident (decision 2): closing Settings must not end the process that
    // shows the next picker.
    QGuiApplication::setQuitOnLastWindowClosed(false);
    return app;
}

int applicationExec()
{
    return QApplication::exec();
}

bool blurBehindRect(QWindow &window, bool enable, int x, int y, int width, int height, double radius)
{
    if (!KWindowEffects::isEffectAvailable(KWindowEffects::BlurBehind)) {
        return false;
    }
    QPainterPath path;
    path.addRoundedRect(QRectF(x, y, width, height), radius, radius);
    KWindowEffects::enableBlurBehind(&window, enable, QRegion(path.toFillPolygon().toPolygon()));
    return true;
}

bool requestActivationToken(QWindow &window, const QString &appId, QObject &receiver)
{
    if (!KWindowSystem::isPlatformWayland()) {
        return false;
    }
    QPointer<QObject> target(&receiver);
    KWaylandExtras::xdgActivationToken(&window, appId).then(&receiver, [target](const QString &token) {
        if (target) {
            QMetaObject::invokeMethod(target.data(), TOKEN_SIGNAL, Q_ARG(QString, token));
        }
    });
    return true;
}

} // namespace wye
