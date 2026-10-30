# 1. Скачать бинарник в песочницу (под любую архитектуру)
wget https://downloads.rclone.org/rclone-current-linux-amd64.zip
unzip rclone-current-linux-amd64.zip
cd rclone-*-linux-amd64

# 2. Настроить переменные окружения с твоими ключами (вводи сразу в консоли)
export RCLONE_CONFIG_S3_TYPE=s3
export RCLONE_CONFIG_S3_ACCESS_KEY_ID=твой_ключ
export RCLONE_CONFIG_S3_SECRET_ACCESS_KEY=твой_секрет
export RCLONE_CONFIG_S3_ENDPOINT=https://s3.ru-msk.vkcloud.ru # пример

# 3. Пушим (если оборвется — запустишь ещё раз, подхватит)
