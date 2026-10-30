wget https://ftp.gnu.org/gnu/xorriso/xorriso-1.4.6.tar.gz
tar -xzf xorriso-1.4.6.tar.gz
cd xorriso-1.4.6

Соберите статический бинарник (без привязки к системным библиотекам). Здесь важно: если у вас нет gcc, придётся сначала установить компилятор другим способом (например, через pacman -S gcc, dnf install gcc, brew install gcc).

bash
