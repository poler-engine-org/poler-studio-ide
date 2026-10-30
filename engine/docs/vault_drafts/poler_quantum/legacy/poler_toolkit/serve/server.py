"""poler_toolkit/serve/server.py — HTTP API Server for POLER Toolkit"""
from http.server import HTTPServer, BaseHTTPRequestHandler
import json

class PolerHTTPHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        response = {"service": "poler-toolkit-api", "version": "2.1.0", "status": "active"}
        self.wfile.write(json.dumps(response).encode())

def run_server(port=8080):
    server = HTTPServer(('0.0.0.0', port), PolerHTTPHandler)
    print(f"POLER Toolkit HTTP Server running on port {port}...")
    server.serve_forever()

if __name__ == '__main__':
    run_server()
