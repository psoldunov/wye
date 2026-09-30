// The only C++ in Wye (docs design B): what neither cxx-qt-lib nor QML
// offers. Every function has a fallback the caller can live with.
#pragma once

#include <memory>

#include <QtCore/QObject>
#include <QtCore/QString>
#include <QtGui/QWindow>
#include <QtWidgets/QApplication>

#include "rust/cxx.h"

namespace wye {

// A QApplication rather than cxx-qt-lib's QGuiApplication: the
// org.kde.desktop Quick Controls style draws with QStyle, which needs one.
// Also sets the desktop file name (the Wayland app id), keeps running when
// the last window closes and, unless QT_QUICK_CONTROLS_STYLE says
// otherwise, uses the org.kde.desktop style.
std::unique_ptr<QApplication> applicationNew(rust::Slice<const rust::String> args,
                                             const QString &desktopFileName,
                                             const QString &displayName);

// QApplication::exec().
int applicationExec();

// Blur behind a rounded rectangle of `window` only, for a panel inside a
// larger transparent window (the picker, PICK-01; KWindowEffects). False
// when the compositor has no blur effect: the caller then draws the panel
// at full opacity (02-picker.md, "No blur available").
bool blurBehindRect(QWindow &window, bool enable, int x, int y, int width, int height, double radius);

// Ask the compositor for an xdg-activation token for `appId`, using the
// last input event `window` received (KWaylandExtras). The token arrives
// later: `receiver`'s signal `activationTokenReady(QString)` is emitted with
// it, or with an empty string when the compositor refused. False, with no
// signal, when the session is not Wayland: the browser then starts without a
// token and the compositor may not raise it.
bool requestActivationToken(QWindow &window, const QString &appId, QObject &receiver);

// `wye-ui --self-test --snapshots`: save every visible QQuickWindow as a PNG,
// the first as `<prefix>.png`, the Mth (M >= 2) as `<prefix>-w<M>.png`, in
// creation order. Returns the files written; a window that cannot be grabbed
// or saved is logged and skipped. Needs QT_QUICK_BACKEND=software offscreen.
rust::Vec<rust::String> saveWindowSnapshots(const QString &prefix);

} // namespace wye
