cat > ~/audio_check.sh << 'EOF'
#!/bin/bash
echo "=== АУДИО-ДИАГНОСТИКА ==="
echo ""
echo "1. Запущенные звуковые сервисы:"
systemctl --user list-units --no-pager '*pipewire*' '*pulse*' '*jack*' '*wire*'
echo ""
echo "2. ALSA устройства:"
aplay -l
echo ""
echo "3. Проприетарный драйвер NVIDIA:"
lsmod | grep nvidia
echo ""
echo "4. Конфигурация ALSA:"
ls -la /etc/asound.conf ~/.asoundrc 2>/dev/null
echo ""
echo "5. Проверка воспроизведения через ALSA:"
timeout 2 speaker-test -D hw:1,3 -c 2 -t sine -f 440 2>&1 | tail -5
echo ""
echo "=== КОНЕЦ ДИАГНОСТИКИ ==="
EOF

chmod +x ~/audio_check.sh
./audio_check.sh
