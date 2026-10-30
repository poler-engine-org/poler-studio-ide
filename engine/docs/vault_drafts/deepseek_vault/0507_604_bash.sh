find . -maxdepth 1 -type f -exec sh -c 'echo "=== {} ==="; cat "{}"' \; > все_вместе.txt

Таким образом, вы получите один файл, содержащий всё содержимое исходных файлов подряд.

DeepThink
Search
AI-generated, for reference only
.-------------------------: vitalij@cachyos-x8664 .+=========================. --------------------- :++===++==================- :++- OS: CachyOS x86_64 :*++====+++++=============- .==: Kernel: Linux 6.18.5-s -*+++=====+***++==========: Uptime: 13 hours, 8 ms =*++++========------------: Packages: 1322 (pacma) =*+++++=====- ... Shell: fish 4.3.3 .+*+++++=-===: .=+++=: Display (Smart TV): 1] :++++=====-==: -*****+ DE: Cinnamon 6.6.5 :++========-=. .=+**+. WM: Muffin (X11) .+==========-. . WM Theme: cinnamon (A) :+++++++====- .--==-. Theme: Adwaita [GTK2/] :++==========. :+++++++: Icons: Papirus [GTK2/] .-===========. =*****+*+ Font: Adwaita Sans (1] .-===========: .+*****+: Cursor: Adwaita (24px) -=======++++:::::::::::::::::::::::::-: .---: Terminal: GNOME Termi1 :======++++====+++******************=. Terminal Font: Adwait) :=====+++==========++++++++++++++*- CPU: Intel(R) Core(TMz .====++==============++++++++++*- GPU: NVIDIA GeForce G] .===+==================+++++++: Memory: 7.28 GiB / 15) .-=======================+++: Swap: 8.00 KiB / 15.5) .......................... Disk (/): 38.40 GiB /s Disk (/run/media/vita4 Disk (/run/media/vita4 Local IP (enp3s0): 194 Locale: uk_UA.UTF-8 /run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/media/Неназвана тека ❯ вывести содержымое всех файлов в этой папку
