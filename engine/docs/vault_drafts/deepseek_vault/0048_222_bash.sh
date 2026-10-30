wget https://ftp.gnu.org/gnu/grub/grub-2.06.tar.xz
tar -xf grub-2.06.tar.xz
cd grub-2.06
./configure --prefix=$HOME/usr --disable-werror
make && make install
