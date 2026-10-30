wget ftp://ftp.gnu.org/gnu/mtools/mtools-4.0.48.tar.gz
tar -xzf mtools-4.0.48.tar.gz
cd mtools-4.0.48
./configure --prefix=$HOME/usr --disable-shared
make && make install

После этого ваши утилиты появятся в ~/usr/bin/. Добавьте этот каталог в PATH:

bash
