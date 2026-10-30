cv::Mat small;
cv::resize(frame, small, cv::Size(320, 240), 0, 0, cv::INTER_NEAREST);
cv::cvtColor(small, gray, cv::COLOR_BGRA2GRAY); // переводим в灰度
