import urllib.request
import json
import socket
import base64
import os
import time

def get_gemini_tab():
    tabs = json.loads(urllib.request.urlopen('http://localhost:9222/json').read())
    for t in tabs:
        if 'gemini' in t.get('url', ''):
            return t
    return None

tab = get_gemini_tab()
print("Gemini Tab:", tab['title'], tab['url'], tab['webSocketDebuggerUrl'])
