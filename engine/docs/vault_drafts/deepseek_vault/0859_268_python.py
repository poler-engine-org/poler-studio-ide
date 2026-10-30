def _handle_analyze_url(self, body):
url = body.get('url')
if not url:
_send_json(self, {'error': 'url required'}, 400)
return
keyword = body.get('keyword', '')
window = int(body.get('window', DEFAULT_WINDOW))
top = int(body.get('top', 10))
phi = float(body.get('phi', 0.85))
kappa = float(body.get('kappa', 1.0))

try:
import requests
from bs4 import BeautifulSoup
except ImportError:
_send_json(self, {'error': 'Missing requests or beautifulsoup4'}, 500)
return

try:
resp = requests.get(url, timeout=30, headers={'User-Agent': 'POLER/6.2'})
resp.raise_for_status()
soup = BeautifulSoup(resp.text, 'html.parser')
# удаляем скрипты и стили
for tag in soup(['script', 'style', 'nav', 'footer', 'header']):
tag.decompose()
text = soup.get_text(separator='\n')
text = '\n'.join(line.strip() for line in text.splitlines() if line.strip())
if not text:
_send_json(self, {'error': 'No text extracted from URL'}, 400)
return
except Exception as e:
_send_json(self, {'error': f'Failed to fetch URL: {str(e)}'}, 500)
return

# Используем существующий анализ
result = analyze_text(text, keyword, window, phi, kappa, top, source_file=url)
_send_json(self, result)

Добавить маршрут в do_POST:

python
