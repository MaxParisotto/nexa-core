#!/usr/bin/env python3
"""
Simple HTTP server for serving the Swagger UI documentation.
"""

import http.server
import socketserver
import webbrowser
from pathlib import Path

PORT = 8088
DIRECTORY = Path(__file__).parent

class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(DIRECTORY), **kwargs)

def main():
    with socketserver.TCPServer(("", PORT), Handler) as httpd:
        print(f"Serving documentation at http://localhost:{PORT}/swagger.html")
        webbrowser.open(f"http://localhost:{PORT}/swagger.html")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down server...")
            httpd.shutdown()

if __name__ == "__main__":
    main() 