#include "opencv2/core.hpp"
#include "OpenCVHelper.h"

// Получаем данные из RenderTarget движка
TArray<FColor> PixelData;
Surface->ReadPixels(PixelData);
// Передаём в OpenCV как Mat (обработка на CPU или GPU через CUDA)
cv::Mat Image(Height, Width, CV_8UC4, PixelData.GetData());
cv::cvtColor(Image, Image, cv::COLOR_RGBA2GRAY);
cv::Canny(Image, edges, 50, 150); // Ищем границы (артефакты)

-
-
29

В) Godot (GDScript + GDNative)
Через нативный код (C++) подключаем OpenCV и обрабатываем кадр:

cpp
