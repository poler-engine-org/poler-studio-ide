echo "vm.swappiness=120" | sudo tee /etc/sysctl.d/zzz-swappiness.conf
sudo sysctl --system
sysctl vm.swappiness
