# Перша група
epubmerge -o part1.epub chapter_p01of3.epub chapter_p02of3.epub chapter_p03of3.epub   # і т.д.
# Потім об'єднай отримані проміжні
epubmerge -o final.epub part*.epub
