static cv::Mat prevGray;
if (!prevGray.empty()) {
cv::Mat diff;
cv::absdiff(gray, prevGray, diff);
cv::threshold(diff, diff, 25, 255, cv::THRESH_BINARY);
int changedPixels = cv::countNonZero(diff);
    if (changedPixels > 500) { // порог движения
// Запускаем детальный анализ
AnalyzeScene(gray);
}
}
