echo "vm.swappiness=120" | sudo tee /etc/sysctl.d/zzz-override.conf
sudo chmod 644 /etc/sysctl.d/zzz-override.conf
sudo sysctl --system
sysctl vm.swappiness
