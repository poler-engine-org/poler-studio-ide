cv::Ptr<cv::Tracker> tracker = cv::TrackerMedianFlow::create();
// Инициализируем на первом кадре выбранным объектом
tracker->init(gray, boundingBox);
// На каждом кадре обновляем позицию
bool ok = tracker->update(gray, boundingBox);
if (ok) {
// Проверяем, не вышла ли рамка за допустимые границы
if (boundingBox.x < 0 || boundingBox.y < 0) {
        SendAlert("Объект вышел за пределы сцены");
}
}
