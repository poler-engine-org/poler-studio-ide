sudo btrfs restore -f -v 2 /dev/sdb4 /mnt/sdc6/restored_home -o subvol=@home

Or skip -v entirely and just run the command. The user wants to see progress; we can just run without -v and it will still output some info.

Let's propose the command without -v first, and if needed add -v later.

Спробуй виконати команду без -v, або з правильним синтаксисом.

bash
