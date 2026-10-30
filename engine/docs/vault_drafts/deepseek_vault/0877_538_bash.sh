sudo btrfs restore -f /dev/sdb4 /mnt/sdc6/restored_home -o subvol=@home

But we saw that gave error. Maybe it's because of the space after -o? Actually, the syntax is -o subvol=@home. It should be fine. Perhaps the error is due to the path /mnt/sdc6/restored_home being inside a filesystem that is still read-only? But they remounted rw. Let's ask them to verify that they can create a file there.

Alternatively, we can try using the btrfs restore with a specific snapshot ID from earlier. For example, ID 302 is the latest snapshot of @. But we need to restore from @home? There are no snapshots of @home, but we can try to restore from the top-level or from the current @home.

Let's try using the -t option to specify the subvolume by ID. Find the ID of @home from earlier list: ID 257. So we can do:

bash
