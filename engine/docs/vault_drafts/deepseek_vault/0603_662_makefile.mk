CC = gcc
CFLAGS = -O2 -Wall

all: constraint_system_v2

constraint_system_v2: system_core_v2.c language_core_v2.c main_v2.c
$(CC) $(CFLAGS) -o constraint_system_v2 system_core_v2.c language_core_v2.c main_v2.c -lm

clean:
rm -f constraint_system_v2 *.o

test: constraint_system_v2
./constraint_system_v2

run1: constraint_system_v2
./constraint_system_v2 1

run2: constraint_system_v2
./constraint_system_v2 2

run3: constraint_system_v2
./constraint_system_v2 3
