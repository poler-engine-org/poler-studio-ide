grep -r "vm.swappiness" /etc/sysctl.d/ /usr/lib/sysctl.d/ 2>/dev/null | sort

Также проверим, нет ли назначения через sysctl -w в каких-нибудь ранних сервисах:

bash
