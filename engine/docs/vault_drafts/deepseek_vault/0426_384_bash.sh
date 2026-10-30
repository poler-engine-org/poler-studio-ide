cd "/home/vitalij/Документи/POLER-DYNAMIS-v5"
find . -type f \( -path "./*" -o -path "./poler_rust_core/*" -o -path "./poler_rust_core/src/*" \) -exec echo '### Файл: {}' \; -exec echo '```' \; -exec cat {} \; -exec echo '```' \; -exec echo '' \;
