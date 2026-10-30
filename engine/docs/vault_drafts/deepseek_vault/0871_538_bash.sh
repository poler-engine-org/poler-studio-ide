sudo btrfs restore -f /dev/sdb4 /mnt/sdc6/restored_home -o subvol=@home

If we want verbose, we can use -v but maybe it should be placed before the path. Try:

bash
