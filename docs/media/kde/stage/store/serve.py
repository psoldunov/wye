"""The made-up web for the store screenshots: an HTTP proxy that the stage's
browsers use for everything, which answers one HTTPS host with page.html and
refuses the rest, so the stage stays offline and the address bar shows a
padlock, not "Not secure".

  serve.py PORT HOST CERT KEY PAGE

CONNECT HOST:443 is answered here, with TLS under CERT (signed by a demo CA
that store.sh adds to each browser's certificate store); every path on HOST
gets PAGE. Any other host gets 403.
"""

import socketserver
import ssl
import sys
from http.server import BaseHTTPRequestHandler

port, host, cert, key, page = sys.argv[1:6]
with open(page, "rb") as file:
    body = file.read()
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(cert, key)


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    tunnelled = False

    def log_message(self, format, *args):  # noqa: A002 # the base class's name
        sys.stderr.write(f"{self.command} {self.path}\n")

    def refuse(self):
        self.send_response(403)
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True

    def do_CONNECT(self):
        if self.tunnelled or self.path != f"{host}:443":
            self.refuse()
            return
        self.send_response(200, "Connection Established")
        self.end_headers()
        self.wfile.flush()
        # The browser now speaks TLS on this connection: serve it from here.
        self.connection = context.wrap_socket(self.connection, server_side=True)
        self.rfile = self.connection.makefile("rb")
        self.wfile = socketserver._SocketWriter(self.connection)
        self.tunnelled = True

    def do_GET(self):
        if not self.tunnelled:
            self.refuse()
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


Server(("127.0.0.1", int(port)), Handler).serve_forever()
