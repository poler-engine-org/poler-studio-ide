export WAYLAND_DISPLAY=waydroid-weston
sudo systemctl start waydroid-container
waydroid session start &
waydroid show-full-ui
