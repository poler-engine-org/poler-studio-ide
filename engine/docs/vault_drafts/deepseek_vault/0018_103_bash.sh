# Скачайте исходники с kernel.org
curl -L -o git.tar.gz https://www.kernel.org/pub/software/scm/git/git-2.45.0.tar.gz
tar -xvzf git.tar.gz
cd git-*

# Установите в папку ~/git (или ~/opt)
make configure
./configure --prefix=$HOME/git
make install

После установки добавьте в ~/.bashrc:

bash
