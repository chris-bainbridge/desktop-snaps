#!/usr/bin/env python3
"""Minimal QtWebEngine shell for the strictly confined pgAdmin 4 snap.

pgAdmin 4's upstream desktop runtime is Electron.  Shipping Electron would
mean downloading a prebuilt Chromium/Electron binary (this repository does not
embed vendor binaries) and disabling its sandbox, which snapd denies to a
strictly confined snap anyway.  Instead this shell reproduces what the upstream
runtime does with the Python backend:

  * pick a free loopback port and a random PGADMIN_INT_KEY,
  * start web/pgAdmin4.py in desktop mode (auto-login, SQLite in the snap's
    private HOME),
  * wait for /misc/ping, then render the UI in QtWebEngine.

QtWebEngine and PyQt6 are staged from the 26.04 archive.  The Chromium
sandbox is disabled for the same reason as in the qutebrowser snap: AppArmor
does not grant unprivileged user namespaces to a strictly confined snap and
snapd denies the allow-sandbox browser-support attribute.
"""

import os
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid

from PyQt6.QtCore import QStandardPaths, QTimer, Qt, QUrl
from PyQt6.QtGui import QDesktopServices, QIcon, QPixmap
from PyQt6.QtWidgets import QApplication, QMainWindow, QMessageBox, QSplashScreen
from PyQt6.QtWebEngineCore import (
    QWebEnginePage,
    QWebEngineProfile,
    QWebEngineSettings,
)
from PyQt6.QtWebEngineWidgets import QWebEngineView

SNAP = os.environ.get("SNAP", "/snap/pgadmin4/current")
WEB_DIR = os.path.join(SNAP, "usr/share/pgadmin4/web")
ICON_PATH = os.path.join(SNAP, "usr/share/icons/hicolor/256x256/apps/pgadmin4.png")
DATA_DIR = os.path.join(os.path.expanduser("~"), ".pgadmin")
LOG_PATH = os.path.join(DATA_DIR, "launcher.log")

_server = None
_log_handle = None
_window = None
_popups = []
_closing = False
_base_url = ""
_start_url = ""


def _tail(path, lines=40):
    try:
        with open(path, "rb") as handle:
            handle.seek(0, os.SEEK_END)
            size = handle.tell()
            handle.seek(max(0, size - 16384))
            text = handle.read().decode("utf-8", "replace")
    except OSError:
        return "(no log)"
    return "\n".join(text.splitlines()[-lines:])


def _free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def _stop_server():
    global _server, _closing
    if _closing:
        return
    _closing = True
    if _server is not None and _server.poll() is None:
        try:
            os.killpg(os.getpgid(_server.pid), signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            _server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(os.getpgid(_server.pid), signal.SIGKILL)
            except ProcessLookupError:
                pass
            _server.wait()
    if _log_handle is not None:
        _log_handle.close()


def _fatal(message):
    QMessageBox.critical(
        None,
        "pgAdmin 4",
        "{}\n\nLog file: {}\n\n{}".format(message, LOG_PATH, _tail(LOG_PATH)),
    )
    _stop_server()
    sys.exit(1)


def _start_server():
    global _server, _log_handle, _base_url, _start_url

    os.makedirs(DATA_DIR, exist_ok=True)
    port = _free_port()
    key = str(uuid.uuid4())
    _base_url = "http://127.0.0.1:{}".format(port)
    _start_url = "{}/?key={}".format(_base_url, key)

    env = os.environ.copy()
    env["PGADMIN_SERVER_MODE"] = "OFF"
    env["PGADMIN_INT_PORT"] = str(port)
    env["PGADMIN_INT_KEY"] = key
    env["PYTHONDONTWRITEBYTECODE"] = "1"

    _log_handle = open(LOG_PATH, "ab", buffering=0)
    _server = subprocess.Popen(
        [sys.executable, "-s", os.path.join(WEB_DIR, "pgAdmin4.py")],
        cwd=WEB_DIR,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=_log_handle,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )

    deadline = time.monotonic() + 40.0
    while time.monotonic() < deadline:
        if _server.poll() is not None:
            _fatal("pgAdmin 4 exited before its web server was ready.")
        try:
            with urllib.request.urlopen(
                "{}/misc/ping?key={}".format(_base_url, key), timeout=2
            ) as response:
                if response.status == 200:
                    return
        except (urllib.error.URLError, OSError):
            pass
        time.sleep(0.2)

    _fatal("Timed out waiting for the pgAdmin 4 web server to start.")


class PgAdminPage(QWebEnginePage):
    """Open non-web schemes in the host browser; keep http(s) in the app."""

    def acceptNavigationRequest(self, url, _nav_type, _is_main_frame):
        if url.scheme() in ("mailto", "tel"):
            QDesktopServices.openUrl(url)
            return False
        if url.scheme() in ("http", "https", "about", "data", "blob", ""):
            return True
        return False

    def createWindow(self, _window_type):
        view = QWebEngineView()
        page = PgAdminPage(self.profile(), view)
        view.setPage(page)
        view.setWindowIcon(QIcon(ICON_PATH))
        view.setWindowTitle("pgAdmin 4")
        view.resize(1100, 800)
        view.show()
        _popups.append(view)
        return page


class MainWindow(QMainWindow):
    def __init__(self, view):
        super().__init__()
        self.setCentralWidget(view)
        self.setWindowTitle("pgAdmin 4")
        self.setWindowIcon(QIcon(ICON_PATH))
        self.resize(1280, 900)

    def toggle_fullscreen(self, request):
        request.accept()
        if request.toggleOn():
            self.showFullScreen()
        else:
            self.showNormal()


def _download(request):
    directory = QStandardPaths.writableLocation(
        QStandardPaths.StandardLocation.DownloadLocation
    )
    os.makedirs(directory, exist_ok=True)
    name = os.path.basename(request.downloadFileName()) or "download"
    stem, suffix = os.path.splitext(name)
    candidate = name
    counter = 1
    while os.path.exists(os.path.join(directory, candidate)):
        candidate = "{} ({}){}".format(stem, counter, suffix)
        counter += 1
    request.setDownloadDirectory(directory)
    request.setDownloadFileName(candidate)
    request.accept()


def _check_server():
    if _closing or _server is None:
        return
    if _server.poll() is not None:
        QMessageBox.critical(
            _window,
            "pgAdmin 4",
            "The pgAdmin 4 server stopped unexpectedly.\n\nLog file: {}\n\n{}".format(
                LOG_PATH, _tail(LOG_PATH)
            ),
        )
        QApplication.instance().quit()


def main():
    global _window

    QApplication.setAttribute(Qt.ApplicationAttribute.AA_ShareOpenGLContexts)
    app = QApplication(sys.argv)
    app.setApplicationName("pgadmin4")
    app.setApplicationDisplayName("pgAdmin 4")
    app.setDesktopFileName("org.pgadmin.pgadmin4")
    app.setWindowIcon(QIcon(ICON_PATH))

    signal.signal(signal.SIGINT, lambda *_: app.quit())
    signal.signal(signal.SIGTERM, lambda *_: app.quit())
    heartbeat = QTimer()
    heartbeat.timeout.connect(lambda: None)
    heartbeat.start(1000)

    splash = None
    pixmap = QPixmap(ICON_PATH)
    if not pixmap.isNull():
        splash = QSplashScreen(pixmap)
        splash.showMessage(
            "Starting pgAdmin 4...",
            Qt.AlignmentFlag.AlignBottom | Qt.AlignmentFlag.AlignHCenter,
            Qt.GlobalColor.white,
        )
        splash.show()
        app.processEvents()

    _start_server()

    # A named profile keeps cookies and local storage in the snap's private
    # data directory instead of an in-memory default profile.
    storage = os.path.join(DATA_DIR, "qtwebengine")
    cache = os.path.join(DATA_DIR, "qtwebengine-cache")
    os.makedirs(storage, exist_ok=True)
    os.makedirs(cache, exist_ok=True)
    profile = QWebEngineProfile("pgadmin4", app)
    profile.setPersistentStoragePath(storage)
    profile.setCachePath(cache)
    profile.setPersistentCookiesPolicy(
        QWebEngineProfile.PersistentCookiesPolicy.ForcePersistentCookies
    )
    profile.downloadRequested.connect(_download)

    page = PgAdminPage(profile)
    view = QWebEngineView()
    view.setPage(page)
    settings = view.settings()
    settings.setAttribute(
        QWebEngineSettings.WebAttribute.LocalContentCanAccessRemoteUrls, False
    )
    settings.setAttribute(
        QWebEngineSettings.WebAttribute.LocalContentCanAccessFileUrls, False
    )
    settings.setAttribute(
        QWebEngineSettings.WebAttribute.JavascriptCanAccessClipboard, True
    )
    settings.setAttribute(
        QWebEngineSettings.WebAttribute.FullScreenSupportEnabled, True
    )

    _window = MainWindow(view)
    page.fullScreenRequested.connect(_window.toggle_fullscreen)
    view.load(QUrl(_start_url))
    _window.show()
    if splash is not None:
        splash.finish(_window)

    app.aboutToQuit.connect(_stop_server)
    checker = QTimer()
    checker.timeout.connect(_check_server)
    checker.start(2000)

    try:
        return app.exec()
    finally:
        _stop_server()


if __name__ == "__main__":
    sys.exit(main())
