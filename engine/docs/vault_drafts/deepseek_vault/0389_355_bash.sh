echo 'ACTION=="change", KERNEL=="zram0", ATTR{initstate}=="1", SYSCTL{vm.swappiness}="120"' | sudo tee /etc/udev/rules.d/99-zram-swappiness-fix.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=block --action=change

После этого проверьте:

bash
