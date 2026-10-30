# 1. Активуємо віртуальне середовище (якщо воно є)
source ~/.litgraph-venv/bin/activate

# 2. Перевіряємо чи spaCy встановлено
python -c "import spacy; print('OK')"

# 3. Якщо помилка — встановлюємо
pip install spacy pymorphy3 numpy scipy scikit-learn
python -m spacy download ru_core_news_sm

# 4. Запускаємо тест
python src-tauri/python/ner_extract.py tests/corpus/01_conflict_scene.md
DeepThink
Search
AI-generated, for reference only
