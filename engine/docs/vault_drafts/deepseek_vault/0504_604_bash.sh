find . -maxdepth 1 -type f -exec sh -c 'file -b --mime-type "{}" | grep -q "^text/" && cat "{}"' \;

Если нужно просмотреть содержимое рекурсивно (включая файлы во вложенных папках), уберите -maxdepth 1:

bash
