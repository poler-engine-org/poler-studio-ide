sudo btrfs restore -f -v /dev/sdb4 /mnt/sdc6/restored_home -o subvol=@home

But the error says -v: negative value is invalid, which suggests it's interpreting -v as an option expecting a number (like -v 2) and when it sees the next argument (the device path) it gets confused. In some btrfs versions, -v can be used with a numeric level (0-2). Possibly we can use -v 2 for max verbosity.

Let's try with -v 2:

bash
