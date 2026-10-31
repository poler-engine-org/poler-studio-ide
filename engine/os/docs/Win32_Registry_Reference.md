# POLER-OS: Win32 Registry Reference & Implementation Guide

> Исследование структуры реестра Windows на основе ReactOS, Wine и оригинальной документации.
> Глубина понимания — сверх Wine и ReactOS, для совместимости с античитами.

---

## 1. SYSTEM Hive (HKLM\SYSTEM)

### 1.1 Select Key

| Value | Type | Value | Notes |
|---|---|---|---|
| Current | REG_DWORD | 1 | Points to ControlSet001 |
| Default | REG_DWORD | 1 | Default for next boot |
| Failed | REG_DWORD | 0 | No failed ControlSet |
| LastKnownGood | REG_DWORD | 1 | Last known good config |

Symbolic links: `CurrentControlSet` → `ControlSet001`, `Clone` (volatile)

### 1.2 CurrentControlSet\Services — CRITICAL для античитов

**Минимальный набор сервисов (Priority 1):**

| Service | Start | Type | Group | ImagePath |
|---|---|---|---|---|
| acpi | 0 | 1 (Kernel) | Boot Bus Extender | system32\drivers\acpi.sys |
| Beep | 1 | 1 (Kernel) | Base | system32\drivers\beep.sys |
| classpnp | 0 | 1 (Kernel) | — | system32\drivers\classpnp.sys |
| cng | 0 | 1 (Kernel) | — | system32\drivers\cng.sys |
| disk | 0 | 1 (Kernel) | — | system32\drivers\disk.sys |
| fltmgr | 0 | 2 (FS) | FSFilter Infrastructure | system32\drivers\fltmgr.sys |
| fs_rec | 1 | 8 (Recognizer) | Boot File System | system32\drivers\fs_rec.sys |
| i8042prt | 1 | 1 (Kernel) | Keyboard Port | system32\drivers\i8042prt.sys |
| kbdclass | 1 | 1 (Kernel) | Keyboard Class | system32\drivers\kbdclass.sys |
| kbdhid | 3 | 1 (Kernel) | Keyboard Port | system32\drivers\kbdhid.sys |
| ksecdd | 0 | 1 (Kernel) | Base | system32\drivers\ksecdd.sys |
| mouclass | 1 | 1 (Kernel) | Pointer Class | system32\drivers\mouclass.sys |
| mouhid | 3 | 1 (Kernel) | Pointer Port | system32\drivers\mouhid.sys |
| MountMgr | 0 | 1 (Kernel) | System Bus Extender | system32\drivers\mountmgr.sys |
| NDIS | 0 | 1 (Kernel) | NDIS Wrapper | system32\drivers\ndis.sys |
| Null | 1 | 1 (Kernel) | Base | system32\drivers\null.sys |
| partmgr | 0 | 1 (Kernel) | — | system32\drivers\partmgr.sys |
| pcw | 0 | 1 (Kernel) | — | system32\drivers\pcw.sys |
| pci | 0 | 1 (Kernel) | Boot Bus Extender | system32\drivers\pci.sys |
| VgaSave | 0 | 1 (Kernel) | Video Save | system32\drivers\vga.sys |
| WDFLDR | 0 | 1 (Kernel) | — | system32\drivers\wdfldr.sys |
| AFD | 1 | 1 (Kernel) | TDI | system32\drivers\afd.sys |
| tcpip | 1 | 1 (Kernel) | TDI | system32\drivers\tcpip.sys |

**Дополнительные сервисы (Win32 compatibility):**

| Service | Start | Type | Group | ImagePath |
|---|---|---|---|---|
| AudioSrv | 3 | 0x10 | AudioGroup | svchost.exe -k LocalService |
| BITS | 3 | 0x20 | — | svchost.exe -k netsvcs |
| DcomLaunch | 2 | 0x20 | Event log | svchost.exe -k DcomLaunch |
| EventLog | 2 | 0x10 | Event Log | system32\eventlog.exe |
| lanmanserver | 2 | 0x20 | — | svchost.exe -k netsvcs |
| lanmanworkstation | 2 | 0x20 | NetworkProvider | svchost.exe -k netsvcs |
| PlugPlay | 3 | 0x20 | PlugPlay | svchost.exe -k DcomLaunch |
| RpcSs | 2 | 0x10 | COM Infrastructure | system32\rpcss.exe |
| Schedule | 2 | 0x20 | SchedulerGroup | svchost.exe -k netsvcs |
| Spooler | 2 | 0x110 | SpoolerGroup | system32\spoolsv.exe |
| Themes | 2 | 0x20 | UIGroup | svchost.exe -k netsvcs |
| Winmgmt | 3 | 0x20 | — | svchost.exe -k netsvcs |

### 1.3 Control\ProductOptions — CRITICAL

| Value | Type | POLER-OS Value | Notes |
|---|---|---|---|
| ProductType | REG_SZ | **WinNT** | НЕ ServerNT! ReactOS ошибается |

### 1.4 Control\Session Manager\KnownDLLs — CRITICAL

**DllDirectory = %SystemRoot%\system32**

POLER-OS должен включать ВСЕ KnownDLLs из реальной Windows 10:

```
advapi32, bcrypt, bcryptPrimitives, cfgmgr32, clbcatq, combase, comdlg32,
cryptbase, devobj, difxapi, dwmapi, gdi32, gdiplus, imagehlp, imm32,
kernel32, kernelbase, msctf, msvcrt, msvcp_win, normaliz, nsi, ntdll,
ntmarta, ole32, oleaut32, powrprof, profapi, propsys, psapi, rpcrt4,
sechost, setupapi, shcore, shell32, shlwapi, sxs, ucrtbase, user32,
win32u, wininet, wldap32, ws2_32, wow64, wow64cpu, wow64win
```

### 1.5 Control\ServiceGroupOrder\List

Полный порядок загрузки групп (из ReactOS, совпадает с Windows):
```
System Reserved, EMS, WdfLoadGroup, Boot Bus Extender, System Bus Extender,
SCSI Miniport, Port, Primary Disk, SCSI Class, SCSI CDROM Class,
FSFilter Infrastructure, FSFilter System, FSFilter Bottom,
FSFilter Copy Protection, FSFilter Security Enhancer, FSFilter Open File,
FSFilter Physical Quota Management, FSFilter Encryption, FSFilter Compression,
FSFilter HSM, FSFilter Cluster File System, FSFilter System Recovery,
FSFilter Quota Management, FSFilter Content Screener, FSFilter Continuous Backup,
FSFilter Replication, FSFilter Anti-Virus, FSFilter Undelete,
FSFilter Activity Monitor, FSFilter Top, Filter, Boot File System,
Base, Pointer Port, Keyboard Port, Pointer Class, Keyboard Class,
Debug, Video Init, Video, Video Save, File System, Event Log,
Streams Drivers, NDIS Wrapper, COM Infrastructure, UIGroup,
LocalValidation, PlugPlay, PNP_TDI, NDIS, TDI, NetBIOSGroup,
ShellSvcGroup, SchedulerGroup, SpoolerGroup, AudioGroup,
SmartCardGroup, NetworkProvider, RemoteValidation, NetDDEGroup,
Parallel arbitrator, Extended Base, PCI Configuration, MS Transactions
```

### 1.6 Control\Class — Device Class GUIDs

| GUID | Class | Description |
|---|---|---|
| {4D36E965-E325-11CE-BFC1-08002BE10318} | CDROM | DVD/CD-ROM drives |
| {4D36E967-E325-11CE-BFC1-08002BE10318} | DiskDrive | Disk drives |
| {4D36E968-E325-11CE-BFC1-08002BE10318} | Display | Display adapters |
| {4D36E969-E325-11CE-BFC1-08002BE10318} | Media | Media changers |
| {4D36E96A-E325-11CE-BFC1-08002BE10318} | HDC | Hard Disk Controllers |
| {4D36E96B-E325-11CE-BFC1-08002BE10318} | Keyboard | Keyboards |
| {4D36E96E-E325-11CE-BFC1-08002BE10318} | Monitor | Monitors |
| {4D36E96F-E325-11CE-BFC1-08002BE10318} | Mouse | Mice/pointing devices |
| {4D36E973-E325-11CE-BFC1-08002BE10318} | NetClient | Network clients |
| {4D36E974-E325-11CE-BFC1-08002BE10318} | NetService | Network services |
| {4D36E975-E325-11CE-BFC1-08002BE10318} | NetTrans | Network transports |
| {4D36E978-E325-11CE-BFC1-08002BE10318} | Ports | COM & LPT ports |
| {4D36E979-E325-11CE-BFC1-08002BE10318} | Printer | Printers |
| {4D36E97B-E325-11CE-BFC1-08002BE10318} | SCSIAdapter | SCSI/RAID controllers |
| {4D36E97D-E325-11CE-BFC1-08002BE10318} | System | System devices |
| {4D36E97E-E325-11CE-BFC1-08002BE10318} | FDC | Floppy disk controllers |
| {4D36E980-E325-11CE-BFC1-08002BE10318} | Battery | Battery devices |
| {50127DC3-0F36-415E-A6CC-4CB3BE910B65} | Processor | Processors |
| {6BDD1FC6-810F-11D0-BEC7-08002BE2092F} | Image | Imaging devices |
| {745A17A0-74D3-11D0-B6FE-00A0C90F57DA} | HIDClass | HID devices |
| {36FC9E60-C465-11CF-8056-444553540000} | USB | USB controllers |
| {EEC12AD6-74AD-4738-8407-E7D3A428DD56} | Bluetooth | Bluetooth |
| {CA3E7359-8D21-4162-9F0B-3F646E7F2C5C} | Sensor | Sensors |
| {E0CBF06C-CD8B-4647-BB8A-263B43F0F974} | Bluetooth | Bluetooth radio |
| {533C5B84-EC70-11D2-9505-00C04F79DEAF} | SoftwareComponent | Software components |
| {71A27CDD-812A-11D0-BEC7-08002BE2092F} | LegacyDriver | Legacy drivers |

---

## 2. SOFTWARE Hive (HKLM\SOFTWARE\Microsoft)

### 2.1 Windows NT\CurrentVersion — CRITICAL

| Value | Type | Value | Notes |
|---|---|---|---|
| CurrentMajorVersionNumber | REG_DWORD | 10 | Win10 major |
| CurrentMinorVersionNumber | REG_DWORD | 0 | Win10 minor |
| CurrentVersion | REG_SZ | "6.3" | Legacy compat |
| CurrentBuild | REG_SZ | "19045" | Win10 22H2 |
| CurrentBuildNumber | REG_SZ | "19045" | Same |
| UBR | REG_DWORD | 5796 | Update Build Revision |
| BuildLab | REG_SZ | "19041.1.amd64fre.vb_release.191206-1406" | **ОБЯЗАТЕЛЬНО** |
| BuildLabEx | REG_SZ | "19041.1.amd64fre.vb_release.191206-1406" | **ОБЯЗАТЕЛЬНО** |
| DisplayVersion | REG_SZ | "22H2" | **ОБЯЗАТЕЛЬНО** |
| ReleaseId | REG_SZ | "2009" | Feature update |
| EditionId | REG_SZ | "Professional" | |
| ProductName | REG_SZ | "Windows 10 Pro" | **НЕ ReactOS!** |
| InstallationType | REG_SZ | "Client" | |
| CurrentType | REG_SZ | "Multiprocessor Free" | |
| DigitalProductId | REG_BINARY | 164 bytes | Валидный blob |
| DigitalProductId4 | REG_BINARY | 0x8D bytes | Валидный blob v4 |
| ProductId | REG_SZ | "XXXXX-XXXXX-XXXXX-XXXXX" | Реалистичный формат |
| InstallDate | REG_DWORD | realistic timestamp | НЕ 0! |
| SystemRoot | REG_SZ | "C:\Windows" | |
| PathName | REG_SZ | "C:\Windows" | |
| BaseBuild | REG_DWORD | 19041 | |
| SoftwareType | REG_SZ | "System" | |
| Composition | REG_DWORD | 1 | |
| ReservesPackageManager | REG_DWORD | 1 | |

### 2.2 Cryptography — CRITICAL

**Провайдеры (отсутствуют в Wine и ReactOS!):**

```
HKLM\SOFTWARE\Microsoft\Cryptography\Defaults\Provider\Microsoft Base Cryptographic Provider v1.0
  ImagePath = "rsaenh.dll", Type = REG_DWORD 1, SigInFile = REG_DWORD 0

HKLM\SOFTWARE\Microsoft\Cryptography\Defaults\Provider\Microsoft Enhanced Cryptographic Provider v1.0
  ImagePath = "rsaenh.dll", Type = REG_DWORD 1, SigInFile = REG_DWORD 0

HKLM\SOFTWARE\Microsoft\Cryptography\Defaults\Provider\Microsoft Enhanced RSA and AES Cryptographic Provider
  ImagePath = "Rsaenh.dll", Type = REG_DWORD 24, SigInFile = REG_DWORD 0

HKLM\SOFTWARE\Microsoft\Cryptography\Defaults\Provider\Microsoft Strong Cryptographic Provider
  ImagePath = "rsaenh.dll", Type = REG_DWORD 1, SigInFile = REG_DWORD 0

HKLM\SOFTWARE\Microsoft\Cryptography\Defaults\Provider\Microsoft Software Key Storage Provider
  (CNG provider)

HKLM\SOFTWARE\Microsoft\Cryptography\RNG
  Seed = REG_BINARY (random bytes)
```

### 2.3 SystemCertificates — CRITICAL

```
HKLM\SOFTWARE\Microsoft\SystemCertificates\CA\Certificates
HKLM\SOFTWARE\Microsoft\SystemCertificates\Disallowed\Certificates
HKLM\SOFTWARE\Microsoft\SystemCertificates\My\Certificates
HKLM\SOFTWARE\Microsoft\SystemCertificates\Root\Certificates
HKLM\SOFTWARE\Microsoft\SystemCertificates\Trust\Certificates
HKLM\SOFTWARE\Microsoft\SystemCertificates\AuthRoot\Certificates
```

### 2.4 Windows\CurrentVersion

```
ProgramFilesDir = "C:\Program Files"
CommonFilesDir = "C:\Program Files\Common Files"
ProductId = реалистичный формат
Policies\System\EnableLUA = REG_DWORD 1
Setup\WindowsFeatures\WindowsMediaVersion = "12.0.7601.18840"
Shell Extensions\Approved
App Paths (для каждого .exe)
```

### 2.5 DirectX

```
HKLM\SOFTWARE\Microsoft\DirectX
  Version = "4.09.00.0904" (REG_SZ)
  InstalledVersion = REG_BINARY 00,00,00,00,09,00,00,00,00
```

### 2.6 OLE

```
HKLM\SOFTWARE\Microsoft\OLE
  EnableDCOM = "Y"
  EnableRemoteConnect = "N"
```

---

## 3. КРИТИЧЕСКИЕ ДЫРЫ в Wine/ReactOS

### 3.1 Мгновенное обнаружение (HIGH)

| Проблема | Wine | ReactOS | Решение POLER-OS |
|---|---|---|---|
| ProductName = "ReactOS" | ✅ | ❌ СТАТЬЯ | "Windows 10 Pro" |
| ProductType = "ServerNT" | ✅ | ❌ | "WinNT" |
| BuildLab отсутствует | ❌ | ❌ | Обязательное поле |
| BuildLabEx отсутствует | ❌ | ❌ | Обязательное поле |
| DisplayVersion отсутствует | ❌ | ❌ | "22H2" |
| DigitalProductId = нули | ❌ | ❌ | Валидный blob |
| ProductId = "12345-oem-..." | ❌ ФЕЙК | ❌ | Реалистичный формат |
| InstallDate = 0 | — | ❌ | Реалистичный timestamp |
| HKCU\Software\Wine | ❌ ДЕТЕКТ | — | Удалить |

### 3.2 Отсутствующие KnownDLLs (HIGH)

Оба не включают: `kernelbase, bcrypt, bcryptPrimitives, cfgmgr32, cryptbase, devobj, dwmapi, sxs, ucrtbase, win32u, msvcp_win, powrprof, profapi, ntdll`

### 3.3 Отсутствующие криптопровайдеры (HIGH)

Ни Wine, ни ReactOS не определяют НИ ОДНОГО провайдера.

### 3.4 Отсутствующие сервисы (HIGH)

Критичные: `fltmgr, ksecdd, cng, WDFLDR, AFD, VgaSave, i8042prt, classpnp, partmgr`

---

## 4. Приоритет реализации

### P0 — Немедленное обнаружение античитом
1. Windows NT\CurrentVersion — ВСЕ значения
2. ProductType = WinNT
3. KnownDLLs — полный список Win10
4. Удалить Wine-специфичные ключи

### P1 — Обнаружение при глубокой проверке
5. Services — минимальный набор (20+ записей)
6. Cryptography\Defaults\Provider — 5 провайдеров
7. SystemCertificates — структура деревьев
8. Device Class GUIDs — все 26+ GUID

### P2 — Совместимость
9. Session Manager\Environment
10. Session Manager\SubSystems
11. ServiceGroupOrder\List
12. FileSystem values
13. NLS code pages
14. Time Zones

### P3 — Антидетект
15. RtlGetVersion согласован с реестром
16. PEB->OSMajorVersion/OSMinorVersion/OSBuildNumber = 10/0/19045
17. RtlGetNtVersionNumbers: build | 0xF0000000
18. NtQuerySystemInformation(SystemWineVersionInformation) = не Wine
19. HKLM\SYSTEM\Setup\SetupType = 0, OOBEInProgress = 0
20. ProductPolicy binary blob

---

## Приложение: Типы сервисов

| Type | Meaning |
|---|---|
| 1 | SERVICE_KERNEL_DRIVER |
| 2 | SERVICE_FILE_SYSTEM_DRIVER |
| 8 | SERVICE_RECOGNIZER_DRIVER |
| 0x10 | SERVICE_WIN32_OWN_PROCESS |
| 0x20 | SERVICE_WIN32_SHARE_PROCESS |
| 0x110 | SERVICE_WIN32_OWN_PROCESS + INTERACTIVE |

| Start | Meaning |
|---|---|
| 0 | SERVICE_BOOT_START |
| 1 | SERVICE_SYSTEM_START |
| 2 | SERVICE_AUTO_START |
| 3 | SERVICE_DEMAND_START |
| 4 | SERVICE_DISABLED |
