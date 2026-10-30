docker run -itd --rm --privileged \
--pull always \
-v ~/android_data:/data \
-p 5555:5555 \
redroid/redroid:12.0.0_64only-latest

Подключитесь к устройству:

bash
