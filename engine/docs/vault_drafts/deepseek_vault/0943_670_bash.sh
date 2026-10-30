systemctl --user stop pipewire wireplumber
PIPEWIRE_DEBUG=5 pipewire &> /tmp/pipewire.log &
