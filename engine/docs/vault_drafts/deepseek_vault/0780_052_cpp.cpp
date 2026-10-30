void AnalyzeScene(const cv::Mat& gray) {
cv::Mat edges;
cv::Canny(gray, edges, 50, 150);
std::vector<std::vector<cv::Point>> contours;
cv::findContours(edges, contours, cv::RETR_EXTERNAL, cv::CHAIN_APPROX_SIMPLE);
// Проверяем контуры на разрывы, неестественные углы и т.д.
for (auto& c : contours) {
double area = cv::contourArea(c);
if (area < 10) continue; // шум
// Если контур имеет самопересечения или слишком острые углы – помечаем как ошибку
if (cv::isContourConvex(c) == false) {
            SendAlert("Некорректный полигон", c);
}
}
}
