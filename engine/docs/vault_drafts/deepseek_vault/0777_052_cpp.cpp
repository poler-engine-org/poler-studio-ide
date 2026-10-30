// Захват кадра из RenderTarget (копирование в ОЗУ)
cv::Mat frame(height, width, CV_8UC4, pixelData);
cv::resize(frame, frame, cv::Size(320, 240)); // снижаем разрешение
cv::cvtColor(frame, frame, cv::COLOR_BGRA2GRAY);

// Легковесный детектор изменений (для отслеживания перемещений)
static cv::Mat prevFrame;
if (!prevFrame.empty()) {
cv::Mat diff;
cv::absdiff(prevFrame, frame, diff);
cv::threshold(diff, diff, 30, 255, cv::THRESH_BINARY);
// Если много изменений - объект двигается
}
prevFrame = frame.clone(); // клонирование, но можно использовать кэш

// Для обнаружения артефактов - простой детектор границ (Canny) - очень быстрый на CPU.
cv::Mat edges;
cv::Canny(frame, edges, 50, 150);
// Анализ: если есть разрывы в контурах - ошибка геометрии.
