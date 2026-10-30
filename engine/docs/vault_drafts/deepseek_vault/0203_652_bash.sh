yay -S anbox-git
sudo modprobe ashmem_linux
systemctl --user start anbox-container-manager
anbox launch --package=org.anbox.appmgr --component=org.anbox.appmgr.AppViewActivity
