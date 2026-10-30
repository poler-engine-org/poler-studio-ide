// Русский текст
input_ru = "переведи 500 рублей ивану";
encode(input_ru) → vectors_ru

// Английский перевод
input_en = "transfer 500 rubles to ivan";
encode(input_en) → vectors_en

// Проверка: vectors_ru[0] должен быть близок к vectors_en[0]
assert(cosine(vectors_ru[0], vectors_en[0]) > 0.9);

Тест 2: Цикл восстановления

c
