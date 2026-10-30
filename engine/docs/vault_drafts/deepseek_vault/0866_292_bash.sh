mkdir -p ~/etheria_clean
find . -type f -exec mv {} ~/etheria_clean/ \;
cd ~/etheria_clean
tar -czvf ~/etheria_full_dump.tar.gz *
