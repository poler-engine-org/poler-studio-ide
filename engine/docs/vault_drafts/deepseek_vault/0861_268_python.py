# poler_skill.py
import json
import os
import urllib.request
import urllib.error
from urllib.parse import urlencode

POLER_API_URL = os.environ.get('POLER_API_URL', 'http://localhost:8000')

def _call_poler(endpoint, data):
url = f"{POLER_API_URL}/{endpoint}"
req = urllib.request.Request(
url,
data=json.dumps(data).encode('utf-8'),
headers={'Content-Type': 'application/json'},
method='POST'
)
try:
with urllib.request.urlopen(req, timeout=60) as resp:
return json.loads(resp.read().decode('utf-8'))
except urllib.error.URLError as e:
return {'error': f'Network error: {str(e)}'}
except Exception as e:
return {'error': f'Unexpected error: {str(e)}'}

def analyze_url(url, keyword=None, top=5, window=3000):
"""
