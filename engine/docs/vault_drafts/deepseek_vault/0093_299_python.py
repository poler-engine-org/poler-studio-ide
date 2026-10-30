import zipfile
import os
import tempfile
import shutil

epubs = sorted([f for f in os.listdir('.') if f.startswith('chapter_') and f.endswith('.epub')])
output = 'merged.epub'

with tempfile.TemporaryDirectory() as tmp:
# Розпаковуємо кожен EPUB у свою підтеку
for i, epub in enumerate(epubs):
with zipfile.ZipFile(epub, 'r') as zf:
zf.extractall(os.path.join(tmp, str(i)))
# Тепер потрібно зібрати все в одну структуру — це складно,
# простіше скористатися epubmerge.
