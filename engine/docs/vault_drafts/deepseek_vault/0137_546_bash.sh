# Створіть теку для монтування
mkdir -p ~/GoogleDrive

# Примонтуйте Google Drive
rclone mount gdrive: ~/GoogleDrive --daemon

# Щоб відмонтувати
fusermount -u ~/GoogleDrive
