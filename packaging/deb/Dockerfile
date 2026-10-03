# Build image for the Wye .deb (packaging/deb/build.sh builds and uses it).
#
# Debian testing, not stable: wye-gtk needs GTK 4.22, libadwaita 1.9 and
# GtkSourceView 5.18, which trixie does not have. The Rust toolchain is
# Debian's own cargo and rustc.
#
#   docker build -t wye-deb-build packaging/deb
#
# The image holds the build dependencies only, read from debian/control's
# Build-Depends, so the two never drift apart. The source is copied into a
# container at build time, and the cargo registry and the target directory
# live in Docker volumes (see build.sh).
FROM debian:testing

ENV DEBIAN_FRONTEND=noninteractive

COPY debian/control /tmp/wye/debian/control

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential \
      ca-certificates \
      lintian \
 && apt-get build-dep -y --no-install-recommends /tmp/wye \
 && rm -rf /var/lib/apt/lists/* /tmp/wye

# cxx-qt-build finds Qt through qmake; cargo's home and target directory are
# the volumes build.sh mounts.
ENV QMAKE=/usr/lib/qt6/bin/qmake \
    CARGO_HOME=/cargo \
    CARGO_TARGET_DIR=/target
