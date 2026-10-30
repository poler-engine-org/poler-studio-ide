# 1. Установка
pip install -r requirements.txt

# 2. Создание тестовой модели
python initialize_ssn.py create --name test_model --size small

# 3. Проверка загрузки
python ssn_loader.py test_model --info

# 4. Тест генерации
python ssn_loader.py test_model --test

# 5. Запуск интерактивного чата
python interactive_conditional_samples.py --model_size small
