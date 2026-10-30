mkdir /tmp/lit-graph
cd /tmp/lit-graph
tar -xvf /home/vitalij/Документи/Нова\ тека/litgraph-desktop/src-tauri/target/release/bundle/deb/lit-graph-0.2.2-1-x86_64.pkg.tar.zst

Потім відредагувати файл .PKGINFO в корені (він буде в каталозі .PKGINFO або просто .PKGINFO? Зазвичай він в корені архіву). Знайти рядок "depend = gtk" і змінити на "depend = gtk3" або "depend = gtk4" в залежності від того, що потрібно. Потім запакувати назад:

bash
