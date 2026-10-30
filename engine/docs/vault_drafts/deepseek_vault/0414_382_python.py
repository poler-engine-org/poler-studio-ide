import ctypes
libcint = ctypes.CDLL("/usr/lib/libcint.so")   # или путь к вашей сборке
print("libcint loaded")

Если появятся новые ошибки — сообщайте.

~/Завантажене/ппппп/pyscf-master/pyscf/geomopt/libcint/build master 17s
❯ cmake .. && make -j
-- The C compiler identification is GNU 16.1.1
-- Detecting C compiler ABI info
-- Detecting C compiler ABI info - done
-- Check for working C compiler: /usr/bin/cc - skipped
-- Detecting C compile features
-- Detecting C compile features - done
Build type RELWITHDEBINFO
-- Performing Test QUADMATH_FOUND
-- Performing Test QUADMATH_FOUND - Success
-- Looking for expl
-- Looking for expl - found
-- Looking for sqrtl
-- Looking for sqrtl - found
P orbitals convention (px, py, pz)
Enable Fortran interface
Enable old cint (version 2) interface
-- Configuring done (0.9s)
-- Generating done (0.0s)
-- Build files have been written to: /home/vitalij/Завантажене/ппппп/pyscf-master/pyscf/geomopt/libcint/build
[  4%] Building C object CMakeFiles/cint.dir/src/fblas.c.o
[  4%] Building C object CMakeFiles/cint.dir/src/cart2sph.c.o
[  6%] Building C object CMakeFiles/cint.dir/src/cint1e.c.o
[  9%] Building C object CMakeFiles/cint.dir/src/cint_bas.c.o
[ 11%] Building C object CMakeFiles/cint.dir/src/g1e.c.o
[ 13%] Building C object CMakeFiles/cint.dir/src/c2f.c.o
[ 16%] Building C object CMakeFiles/cint.dir/src/cint2e.c.o
[ 18%] Building C object CMakeFiles/cint.dir/src/eigh.c.o
[ 20%] Building C object CMakeFiles/cint.dir/src/optimizer.c.o
[ 23%] Building C object CMakeFiles/cint.dir/src/g2e.c.o
[ 25%] Building C object CMakeFiles/cint.dir/src/g2c2e.c.o
[ 27%] Building C object CMakeFiles/cint.dir/src/cint2c2e.c.o
[ 30%] Building C object CMakeFiles/cint.dir/src/misc.c.o
[ 34%] Building C object CMakeFiles/cint.dir/src/rys_roots.c.o
[ 32%] Building C object CMakeFiles/cint.dir/src/fmt.c.o
[ 37%] Building C object CMakeFiles/cint.dir/src/rys_wheeler.c.o
[ 39%] Building C object CMakeFiles/cint.dir/src/find_roots.c.o
[ 41%] Building C object CMakeFiles/cint.dir/src/g3c1e.c.o
[ 44%] Building C object CMakeFiles/cint.dir/src/cint1e_a.c.o
[ 46%] Building C object CMakeFiles/cint.dir/src/cint3c1e.c.o
[ 48%] Building C object CMakeFiles/cint.dir/src/cint3c2e.c.o
[ 51%] Building C object CMakeFiles/cint.dir/src/autocode/breit1.c.o
[ 53%] Building C object CMakeFiles/cint.dir/src/g3c2e.c.o
[ 55%] Building C object CMakeFiles/cint.dir/src/cint3c1e_a.c.o
[ 58%] Building C object CMakeFiles/cint.dir/src/cint1e_grids.c.o
[ 60%] Building C object CMakeFiles/cint.dir/src/breit.c.o
[ 62%] Building C object CMakeFiles/cint.dir/src/g1e_grids.c.o
[ 65%] Building C object CMakeFiles/cint.dir/src/autocode/int3c2e.c.o
[ 76%] Building C object CMakeFiles/cint.dir/src/autocode/hess.c.o
[ 79%] Building C object CMakeFiles/cint.dir/src/autocode/deriv4.c.o
[ 67%] Building C object CMakeFiles/cint.dir/src/autocode/grad2.c.o
[ 81%] Building C object CMakeFiles/cint.dir/src/autocode/deriv3.c.o
[ 69%] Building C object CMakeFiles/cint.dir/src/autocode/dkb.c.o
[ 83%] Building C object CMakeFiles/cint.dir/src/autocode/lresc.c.o
[ 72%] Building C object CMakeFiles/cint.dir/src/autocode/gaunt1.c.o
[ 74%] Building C object CMakeFiles/cint.dir/src/autocode/int1e_grids1.c.o
[ 86%] Building C object CMakeFiles/cint.dir/src/autocode/int3c1e.c.o
[ 88%] Building C object CMakeFiles/cint.dir/src/autocode/grad1.c.o
[ 90%] Building C object CMakeFiles/cint.dir/src/autocode/intor1.c.o
[ 93%] Building C object CMakeFiles/cint.dir/src/autocode/intor4.c.o
[ 95%] Building C object CMakeFiles/cint.dir/src/autocode/intor2.c.o
[ 97%] Building C object CMakeFiles/cint.dir/src/autocode/intor3.c.o
[100%] Linking C shared library libcint.so
[100%] Built target cint

~/Завантажене/ппппп/pyscf-master/pyscf/geomopt/libcint/build master* 22s
❯
Thought for 2 seconds
