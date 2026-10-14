#!/usr/bin/env python3
"""
HTTP server for CleanCore Studio Comparison Gallery.
Serves docs/design directory, defaulting to studio.html on root path.
"""

import http.server
import socketserver
from pathlib import Path

DIRECTORY = Path(__file__).resolve().parent.parent / "docs" / "design"

class StudioGalleryHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(DIRECTORY), **kwargs)

    def do_GET(self):
        if self.path in ("/", "/index.html"):
            self.path = "/studio.html"
        return super().do_GET()

if __name__ == "__main__":
    PORT = 5002
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("127.0.0.1", PORT), StudioGalleryHandler) as httpd:
        print(f"Studio Gallery serving at http://127.0.0.1:{PORT}")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            pass
