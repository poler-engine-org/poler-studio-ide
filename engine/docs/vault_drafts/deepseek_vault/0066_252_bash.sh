sudo pacman -S qemu virt-manager

Але давайте спочатку подивимось лог VirtualBox — 99% проблем вирішуються зміною одного параметра. Чекаю на вивід!

00:00:00.019520 VirtualBox VM 7.2.12 r174389 linux.amd64 (Jul  1 2026 04:31:00) release log
00:00:00.019522 Log opened 2026-07-07T18:51:03.220410000Z
00:00:00.019523 Build Type: release
00:00:00.019524 OS Product: Linux
00:00:00.019525 OS Release: 7.0.11-1-cachyos
00:00:00.019526 OS Version: #1 SMP PREEMPT_DYNAMIC Fri, 05 Jun 2026 16:36:35 +0000
00:00:00.019542 DMI Product Name: To be filled by O.E.M.
00:00:00.019548 DMI Product Version: To be filled by O.E.M.
00:00:00.019552 Firmware type: UEFI
00:00:00.019754 Secure Boot: Disabled
00:00:00.019781 Host RAM: 15948MB (15.5GB) total, 12084MB (11.8GB) available
00:00:00.019783 Executable: /usr/lib/virtualbox/VBoxHeadless
00:00:00.019783 Process ID: 7561
00:00:00.019783 Package type: LINUX_64BITS_GENERIC (OSE)
00:00:00.024050 Installed Extension Packs:
00:00:00.024059   None installed!
00:00:00.024768 Console: Machine state changed to 'Starting'
00:00:00.032723 SUP: seg #0: R   0x00000000 LB 0x0004c000
00:00:00.032747 SUP: seg #1: R X 0x0004c000 LB 0x00260000
00:00:00.032750 SUP: seg #2: R   0x002ac000 LB 0x00075000
00:00:00.032753 SUP: seg #3: RW  0x00321000 LB 0x0002db48
00:00:00.034926 SUP: Loaded VMMR0.r0 (/usr/lib/virtualbox/VMMR0.r0) at 0xXXXXXXXXXXXXXXXX - ModuleInit at XXXXXXXXXXXXXXXX and ModuleTerm at XXXXXXXXXXXXXXXX
00:00:00.034952 SUP: VMMR0EntryEx located at XXXXXXXXXXXXXXXX and VMMR0EntryFast at XXXXXXXXXXXXXXXX
00:00:00.038032 Guest architecture: x86
00:00:00.038134 Guest OS type: 'Other'
00:00:00.039448 fHMForced=true - No raw-mode support in this build!
00:00:00.039458 Using execution engine 1
00:00:00.048560 File system of '/home/vitalij/VirtualBox VMs/poler-os64-minimal/poler-os64-minimal.vdi' is ext4
00:00:00.050233 File system of '/home/vitalij/Стільниця/разроботка/Нова тека/ZCodeProject/poler-os-work/poler-os64-minimal.iso' (DVD) is ext4
00:00:00.057695 Shared Clipboard: Service loaded
00:00:00.057709 Shared Clipboard: Mode: Off
00:00:00.057810 Shared Clipboard: Service running in headless mode
00:00:00.058405 Drag and drop service loaded
00:00:00.058409 Drag and drop mode: Off
00:00:00.064700 Audio: Detected default audio driver type is 'ALSAAudio'
00:00:00.071773 ************************* CFGM dump *************************
00:00:00.071775 [/] (level 0)
00:00:00.071777   CpuExecutionCap   <integer> = 0x0000000000000064 (100)
00:00:00.071779   EnablePAE         <integer> = 0x0000000000000000 (0)
00:00:00.071780   HMEnabled         <integer> = 0x0000000000000001 (1)
00:00:00.071781   MemBalloonSize    <integer> = 0x0000000000000000 (0, 0 B)
00:00:00.071782   Name              <string>  = "poler-os64-minimal" (cb=19)
00:00:00.071783   NumCPUs           <integer> = 0x0000000000000004 (4)
00:00:00.071784   PageFusionAllowed <integer> = 0x0000000000000000 (0)
00:00:00.071784   RamHoleSize       <integer> = 0x0000000020000000 (536 870 912, 512.0 MiB)
00:00:00.071786   RamSize           <integer> = 0x0000000100000000 (4 294 967 296, 4.0 GiB)
00:00:00.071787   TimerMillies      <integer> = 0x000000000000000a (10)
00:00:00.071788   UUID              <bytes>   = "6f f8 c8 df 9e a1 af 4d 9b 7c 7d bb d7 f0 9e e8" (cb=16)
00:00:00.071790
00:00:00.071791 [/CPUM/] (level 1)
00:00:00.071792   Enable64bit        <integer> = 0x0000000000000000 (0)
00:00:00.071792   GuestCpuName       <string>  = "host" (cb=5)
00:00:00.071793   NestedHWVirt       <integer> = 0x0000000000000000 (0)
00:00:00.071793   PortableCpuIdLevel <integer> = 0x0000000000000000 (0)
00:00:00.071794   SpecCtrl           <integer> = 0x0000000000000000 (0)
00:00:00.071795
00:00:00.071795 [/CPUM/IsaExts/] (level 2)
00:00:00.071795
00:00:00.071796 [/DBGC/] (level 1)
00:00:00.071796   GlobalInitScript <string>  = "/home/vitalij/.config/VirtualBox/dbgc-init" (cb=43)
00:00:00.071797   HistoryFile      <string>  = "/home/vitalij/.config/VirtualBox/dbgc-history" (cb=46)
00:00:00.071797   LocalInitScript  <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/dbgc-init" (cb=58)
00:00:00.071798
00:00:00.071798 [/DBGF/] (level 1)
00:00:00.071799   Path <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/debug/;/home/vitalij/VirtualBox VMs/poler-os64-minimal/;cache*/home/vitalij/VirtualBox VMs/poler-os64-minimal/dbgcache/;/home/vitalij/" (cb=183)
00:00:00.071799
00:00:00.071800 [/Devices/] (level 1)
00:00:00.071800
00:00:00.071800 [/Devices/3c501/] (level 2)
00:00:00.071801
00:00:00.071801 [/Devices/8237A/] (level 2)
00:00:00.071802
00:00:00.071802 [/Devices/8237A/0/] (level 3)
00:00:00.071803   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.071803
00:00:00.071804 [/Devices/VMMDev/] (level 2)
00:00:00.071804
00:00:00.071804 [/Devices/VMMDev/0/] (level 3)
00:00:00.071805   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.071806   PCIDeviceNo   <integer> = 0x0000000000000004 (4)
00:00:00.071806   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.071807   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.071808
00:00:00.071808 [/Devices/VMMDev/0/Config/] (level 4)
00:00:00.071809   GuestCoreDumpDir <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/Snapshots" (cb=58)
00:00:00.071809
00:00:00.071809 [/Devices/VMMDev/0/LUN#0/] (level 4)
00:00:00.071810   Driver <string>  = "HGCM" (cb=5)
00:00:00.071810
00:00:00.071811 [/Devices/VMMDev/0/LUN#0/Config/] (level 5)
00:00:00.071812
00:00:00.071812 [/Devices/VMMDev/0/LUN#999/] (level 4)
00:00:00.071812   Driver <string>  = "MainStatus" (cb=11)
00:00:00.071813
00:00:00.071813 [/Devices/VMMDev/0/LUN#999/Config/] (level 5)
00:00:00.071814   First                <integer> = 0x0000000000000000 (0)
00:00:00.071815   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.071815   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.071816   iLedSet              <integer> = 0x0000000000000005 (5)
00:00:00.071817
00:00:00.071817 [/Devices/acpi/] (level 2)
00:00:00.071817
00:00:00.071818 [/Devices/acpi/0/] (level 3)
00:00:00.071818   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.071819   PCIDeviceNo   <integer> = 0x0000000000000007 (7)
00:00:00.071819   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.071820   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.071821
00:00:00.071821 [/Devices/acpi/0/Config/] (level 4)
00:00:00.071822   CpuHotPlug          <integer> = 0x0000000000000000 (0)
00:00:00.071822   FdcEnabled          <integer> = 0x0000000000000000 (0)
00:00:00.071823   HostBusPciAddress   <integer> = 0x0000000000000000 (0)
00:00:00.071823   HpetEnabled         <integer> = 0x0000000000000000 (0)
00:00:00.071824   IOAPIC              <integer> = 0x0000000000000001 (1)
00:00:00.071825   IocPciAddress       <integer> = 0x0000000000010000 (65 536)
00:00:00.071826   NumCPUs             <integer> = 0x0000000000000004 (4)
00:00:00.071826   Parallel0IoPortBase <integer> = 0x0000000000000000 (0)
00:00:00.071827   Parallel0Irq        <integer> = 0x0000000000000000 (0)
00:00:00.071827   Parallel1IoPortBase <integer> = 0x0000000000000000 (0)
00:00:00.071828   Parallel1Irq        <integer> = 0x0000000000000000 (0)
00:00:00.071828   Serial0IoPortBase   <integer> = 0x0000000000000000 (0)
00:00:00.071829   Serial0Irq          <integer> = 0x0000000000000000 (0)
00:00:00.071830   Serial1IoPortBase   <integer> = 0x0000000000000000 (0)
00:00:00.071830   Serial1Irq          <integer> = 0x0000000000000000 (0)
00:00:00.071831   ShowCpu             <integer> = 0x0000000000000001 (1)
00:00:00.071831   ShowRtc             <integer> = 0x0000000000000000 (0)
00:00:00.071832   SmcEnabled          <integer> = 0x0000000000000000 (0)
00:00:00.071833
00:00:00.071833 [/Devices/acpi/0/LUN#0/] (level 4)
00:00:00.071834   Driver <string>  = "ACPIHost" (cb=9)
00:00:00.071834
00:00:00.071834 [/Devices/acpi/0/LUN#0/Config/] (level 5)
00:00:00.071835
00:00:00.071835 [/Devices/acpi/0/LUN#1/] (level 4)
00:00:00.071836   Driver <string>  = "ACPICpu" (cb=8)
00:00:00.071836
00:00:00.071837 [/Devices/acpi/0/LUN#1/Config/] (level 5)
00:00:00.071837
00:00:00.071838 [/Devices/acpi/0/LUN#2/] (level 4)
00:00:00.071838   Driver <string>  = "ACPICpu" (cb=8)
00:00:00.071839
00:00:00.071839 [/Devices/acpi/0/LUN#2/Config/] (level 5)
00:00:00.071840
00:00:00.071840 [/Devices/acpi/0/LUN#3/] (level 4)
00:00:00.071841   Driver <string>  = "ACPICpu" (cb=8)
00:00:00.071841
00:00:00.071841 [/Devices/acpi/0/LUN#3/Config/] (level 5)
00:00:00.071842
00:00:00.071842 [/Devices/apic/] (level 2)
00:00:00.071843
00:00:00.071843 [/Devices/apic/0/] (level 3)
00:00:00.071844   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.071844
00:00:00.071844 [/Devices/apic/0/Config/] (level 4)
00:00:00.071845   IOAPIC  <integer> = 0x0000000000000001 (1)
00:00:00.071846   Mode    <integer> = 0x0000000000000002 (2)
00:00:00.071846   NumCPUs <integer> = 0x0000000000000004 (4)
00:00:00.071847
00:00:00.071847 [/Devices/dp8390/] (level 2)
00:00:00.071848
00:00:00.071848 [/Devices/e1000/] (level 2)
00:00:00.071848
00:00:00.071849 [/Devices/i8254/] (level 2)
00:00:00.071849
00:00:00.071849 [/Devices/i8254/0/] (level 3)
00:00:00.071850
00:00:00.071850 [/Devices/i8254/0/Config/] (level 4)
00:00:00.071851
00:00:00.071851 [/Devices/i8259/] (level 2)
00:00:00.071852
00:00:00.071852 [/Devices/i8259/0/] (level 3)
00:00:00.071853   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.071853
00:00:00.071853 [/Devices/i8259/0/Config/] (level 4)
00:00:00.071854
00:00:00.071854 [/Devices/ichac97/] (level 2)
00:00:00.071855
00:00:00.071855 [/Devices/ichac97/0/] (level 3)
00:00:00.071856   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.071857   PCIDeviceNo   <integer> = 0x0000000000000005 (5)
00:00:00.071857   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.071858   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.071858
00:00:00.071859 [/Devices/ichac97/0/AudioConfig/] (level 4)
00:00:00.071859
00:00:00.071860 [/Devices/ichac97/0/Config/] (level 4)
00:00:00.071860   Codec        <string>  = "STAC9700" (cb=9)
00:00:00.071861   DebugEnabled <integer> = 0x0000000000000000 (0)
00:00:00.071861
00:00:00.071862 [/Devices/ichac97/0/LUN#0/] (level 4)
00:00:00.071862   Driver <string>  = "AUDIO" (cb=6)
00:00:00.071863
00:00:00.071863 [/Devices/ichac97/0/LUN#0/AttachedDriver/] (level 5)
00:00:00.071864   Driver <string>  = "ALSAAudio" (cb=10)
00:00:00.071864
00:00:00.071865 [/Devices/ichac97/0/LUN#0/AttachedDriver/Config/] (level 6)
00:00:00.071866
00:00:00.071866 [/Devices/ichac97/0/LUN#0/Config/] (level 5)
00:00:00.071867   DriverName    <string>  = "ALSAAudio" (cb=10)
00:00:00.071867   InputEnabled  <integer> = 0x0000000000000000 (0)
00:00:00.071868   OutputEnabled <integer> = 0x0000000000000001 (1)
00:00:00.071868
00:00:00.071869 [/Devices/ichac97/0/LUN#1/] (level 4)
00:00:00.071869   Driver <string>  = "AUDIO" (cb=6)
00:00:00.071870
00:00:00.071870 [/Devices/ichac97/0/LUN#2/] (level 4)
00:00:00.071871   Driver <string>  = "AUDIO" (cb=6)
00:00:00.071871
00:00:00.071871 [/Devices/ioapic/] (level 2)
00:00:00.071872
00:00:00.071872 [/Devices/ioapic/0/] (level 3)
00:00:00.071873   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.071873
00:00:00.071873 [/Devices/ioapic/0/Config/] (level 4)
00:00:00.071874   NumCPUs <integer> = 0x0000000000000004 (4)
00:00:00.071875
00:00:00.071875 [/Devices/mc146818/] (level 2)
00:00:00.071875
00:00:00.071876 [/Devices/mc146818/0/] (level 3)
00:00:00.071876
00:00:00.071877 [/Devices/mc146818/0/Config/] (level 4)
00:00:00.071877   UseUTC <integer> = 0x0000000000000000 (0)
00:00:00.071878
00:00:00.071878 [/Devices/parallel/] (level 2)
00:00:00.071879
00:00:00.071879 [/Devices/pcarch/] (level 2)
00:00:00.071879
00:00:00.071880 [/Devices/pcarch/0/] (level 3)
00:00:00.071880   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.071881
00:00:00.071881 [/Devices/pcarch/0/Config/] (level 4)
00:00:00.071882
00:00:00.071882 [/Devices/pcbios/] (level 2)
00:00:00.071883
00:00:00.071883 [/Devices/pcbios/0/] (level 3)
00:00:00.071883   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.071884
00:00:00.071884 [/Devices/pcbios/0/Config/] (level 4)
00:00:00.071886   APIC            <integer> = 0x0000000000000001 (1)
00:00:00.071886   BootDevice0     <string>  = "FLOPPY" (cb=7)
00:00:00.071887   BootDevice1     <string>  = "DVD" (cb=4)
00:00:00.071887   BootDevice2     <string>  = "IDE" (cb=4)
00:00:00.071888   BootDevice3     <string>  = "NONE" (cb=5)
00:00:00.071888   DmiSystemSerial <string>  = "VirtualBox-<DmiSystemUuid>" (cb=27)
00:00:00.071889   FloppyDevice    <string>  = "i82078" (cb=7)
00:00:00.071889   HardDiskDevice  <string>  = "piix3ide" (cb=9)
00:00:00.071890   IOAPIC          <integer> = 0x0000000000000001 (1)
00:00:00.071890   McfgBase        <integer> = 0x0000000000000000 (0)
00:00:00.071891   McfgLength      <integer> = 0x0000000000000000 (0)
00:00:00.071891   NumCPUs         <integer> = 0x0000000000000004 (4)
00:00:00.071892   PXEDebug        <integer> = 0x0000000000000000 (0)
00:00:00.071893   UUID            <bytes>   = "6f f8 c8 df 9e a1 af 4d 9b 7c 7d bb d7 f0 9e e8" (cb=16)
00:00:00.071894   UuidLe          <integer> = 0x0000000000000001 (1)
00:00:00.071895
00:00:00.071895 [/Devices/pcbios/0/Config/NetBoot/] (level 5)
00:00:00.071896
00:00:00.071896 [/Devices/pcbios/0/Config/NetBoot/0/] (level 6)
00:00:00.071897   NIC           <integer> = 0x0000000000000000 (0)
00:00:00.071898   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.071898   PCIDeviceNo   <integer> = 0x0000000000000003 (3)
00:00:00.071899   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.071900
00:00:00.071900 [/Devices/pci/] (level 2)
00:00:00.071900
00:00:00.071901 [/Devices/pci/0/] (level 3)
00:00:00.071901   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.071902
00:00:00.071902 [/Devices/pci/0/Config/] (level 4)
00:00:00.071903   IOAPIC <integer> = 0x0000000000000001 (1)
00:00:00.071903
00:00:00.071903 [/Devices/pcibridge/] (level 2)
00:00:00.071904
00:00:00.071904 [/Devices/pckbd/] (level 2)
00:00:00.071905
00:00:00.071905 [/Devices/pckbd/0/] (level 3)
00:00:00.071906   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.071906
00:00:00.071906 [/Devices/pckbd/0/Config/] (level 4)
00:00:00.071907
00:00:00.071907 [/Devices/pckbd/0/LUN#0/] (level 4)
00:00:00.071908   Driver <string>  = "KeyboardQueue" (cb=14)
00:00:00.071908
00:00:00.071909 [/Devices/pckbd/0/LUN#0/AttachedDriver/] (level 5)
00:00:00.071910   Driver <string>  = "MainKeyboard" (cb=13)
00:00:00.071910
00:00:00.071910 [/Devices/pckbd/0/LUN#0/Config/] (level 5)
00:00:00.071911   QueueSize <integer> = 0x0000000000000040 (64, 64 B)
00:00:00.071912
00:00:00.071912 [/Devices/pckbd/0/LUN#1/] (level 4)
00:00:00.071913   Driver <string>  = "MouseQueue" (cb=11)
00:00:00.071913
00:00:00.071913 [/Devices/pckbd/0/LUN#1/AttachedDriver/] (level 5)
00:00:00.071914   Driver <string>  = "MainMouse" (cb=10)
00:00:00.071915
00:00:00.071915 [/Devices/pckbd/0/LUN#1/Config/] (level 5)
00:00:00.071916   QueueSize <integer> = 0x0000000000000080 (128, 128 B)
00:00:00.071916
00:00:00.071917 [/Devices/pcnet/] (level 2)
00:00:00.071917
00:00:00.071917 [/Devices/pcnet/0/] (level 3)
00:00:00.071918   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.071919   PCIDeviceNo   <integer> = 0x0000000000000003 (3)
00:00:00.071919   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.071920   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.071920
00:00:00.071921 [/Devices/pcnet/0/Config/] (level 4)
00:00:00.071921   CableConnected <integer> = 0x0000000000000001 (1)
00:00:00.071922   ChipType       <string>  = "Am79C973" (cb=9)
00:00:00.071922   LineSpeed      <integer> = 0x0000000000000000 (0)
00:00:00.071923   MAC            <bytes>   = "08 00 27 62 22 16" (cb=6)
00:00:00.071924
00:00:00.071924 [/Devices/pcnet/0/LUN#0/] (level 4)
00:00:00.071925   Driver <string>  = "NAT" (cb=4)
00:00:00.071925
00:00:00.071925 [/Devices/pcnet/0/LUN#0/Config/] (level 5)
00:00:00.071927   AliasMode          <integer> = 0x0000000000000000 (0)
00:00:00.071927   DNSProxy           <integer> = 0x0000000000000000 (0)
00:00:00.071928   EnableTFTP         <integer> = 0x0000000000000000 (0)
00:00:00.071928   ForwardBroadcast   <integer> = 0x0000000000000000 (0)
00:00:00.071929   LocalhostReachable <integer> = 0x0000000000000001 (1)
00:00:00.071929   Network            <string>  = "10.0.2.0/24" (cb=12)
00:00:00.071930   PassDomain         <integer> = 0x0000000000000001 (1)
00:00:00.071931   UseHostResolver    <integer> = 0x0000000000000000 (0)
00:00:00.071931
00:00:00.071931 [/Devices/pcnet/0/LUN#999/] (level 4)
00:00:00.071932   Driver <string>  = "MainStatus" (cb=11)
00:00:00.071933
00:00:00.071933 [/Devices/pcnet/0/LUN#999/Config/] (level 5)
00:00:00.071934   First                <integer> = 0x0000000000000000 (0)
00:00:00.071934   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.071935   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.071935   iLedSet              <integer> = 0x0000000000000004 (4)
00:00:00.071936
00:00:00.071936 [/Devices/piix3ide/] (level 2)
00:00:00.071937
00:00:00.071937 [/Devices/piix3ide/0/] (level 3)
00:00:00.071938   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.071939   PCIDeviceNo   <integer> = 0x0000000000000001 (1)
00:00:00.071939   PCIFunctionNo <integer> = 0x0000000000000001 (1)
00:00:00.071940   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.071940
00:00:00.071940 [/Devices/piix3ide/0/Config/] (level 4)
00:00:00.071941   Type <string>  = "PIIX4" (cb=6)
00:00:00.071942
00:00:00.071942 [/Devices/piix3ide/0/LUN#0/] (level 4)
00:00:00.071943   Driver <string>  = "VD" (cb=3)
00:00:00.071943
00:00:00.071943 [/Devices/piix3ide/0/LUN#0/Config/] (level 5)
00:00:00.071944   Format    <string>  = "VDI" (cb=4)
00:00:00.071945   Mountable <integer> = 0x0000000000000000 (0)
00:00:00.071945   Path      <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/poler-os64-minimal.vdi" (cb=71)
00:00:00.071946   Type      <string>  = "HardDisk" (cb=9)
00:00:00.071946
00:00:00.071946 [/Devices/piix3ide/0/LUN#0/Config/VDConfig/] (level 6)
00:00:00.071947   AllocationBlockSize <string>  = "1048576" (cb=8)
00:00:00.071948
00:00:00.071948 [/Devices/piix3ide/0/LUN#2/] (level 4)
00:00:00.071949   Driver <string>  = "VD" (cb=3)
00:00:00.071949
00:00:00.071949 [/Devices/piix3ide/0/LUN#2/Config/] (level 5)
00:00:00.071950   Format    <string>  = "RAW" (cb=4)
00:00:00.071951   Mountable <integer> = 0x0000000000000001 (1)
00:00:00.071951   Path      <string>  = "/home/vitalij/Стільниця/разроботка/Нова тека/ZCodeProject/poler-os-work/poler-os64-minimal.iso" (cb=122)
00:00:00.071952   ReadOnly  <integer> = 0x0000000000000001 (1)
00:00:00.071952   Type      <string>  = "DVD" (cb=4)
00:00:00.071953
00:00:00.071953 [/Devices/piix3ide/0/LUN#999/] (level 4)
00:00:00.071954   Driver <string>  = "MainStatus" (cb=11)
00:00:00.071954
00:00:00.071954 [/Devices/piix3ide/0/LUN#999/Config/] (level 5)
00:00:00.071955   DeviceInstance       <string>  = "piix3ide/0" (cb=11)
00:00:00.071956   First                <integer> = 0x0000000000000000 (0)
00:00:00.071956   HasMediumAttachments <integer> = 0x0000000000000001 (1)
00:00:00.071957   Last                 <integer> = 0x0000000000000003 (3)
00:00:00.071958   iLedSet              <integer> = 0x0000000000000003 (3)
00:00:00.071958
00:00:00.071958 [/Devices/serial/] (level 2)
00:00:00.071959
00:00:00.071959 [/Devices/usb-ehci/] (level 2)
00:00:00.071960
00:00:00.071960 [/Devices/usb-ehci/0/] (level 3)
00:00:00.071961   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.071962   PCIDeviceNo   <integer> = 0x000000000000000b (11)
00:00:00.071962   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.071963   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.071963
00:00:00.071964 [/Devices/usb-ehci/0/Config/] (level 4)
00:00:00.071964
00:00:00.071965 [/Devices/usb-ehci/0/LUN#0/] (level 4)
00:00:00.071965   Driver <string>  = "VUSBRootHub" (cb=12)
00:00:00.071966
00:00:00.071966 [/Devices/usb-ehci/0/LUN#0/Config/] (level 5)
00:00:00.071967
00:00:00.071967 [/Devices/usb-ehci/0/LUN#999/] (level 4)
00:00:00.071968   Driver <string>  = "MainStatus" (cb=11)
00:00:00.071968
00:00:00.071968 [/Devices/usb-ehci/0/LUN#999/Config/] (level 5)
00:00:00.071969   First                <integer> = 0x0000000000000000 (0)
00:00:00.071970   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.071970   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.071971   iLedSet              <integer> = 0x0000000000000002 (2)
00:00:00.071972
00:00:00.071972 [/Devices/usb-ohci/] (level 2)
00:00:00.071972
00:00:00.071973 [/Devices/usb-ohci/0/] (level 3)
00:00:00.071973   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.071974   PCIDeviceNo   <integer> = 0x0000000000000006 (6)
00:00:00.071974   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.071975   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.071976
00:00:00.071976 [/Devices/usb-ohci/0/Config/] (level 4)
00:00:00.071976
00:00:00.071977 [/Devices/usb-ohci/0/LUN#0/] (level 4)
00:00:00.071977   Driver <string>  = "VUSBRootHub" (cb=12)
00:00:00.071978
00:00:00.071978 [/Devices/usb-ohci/0/LUN#0/Config/] (level 5)
00:00:00.071979
00:00:00.071979 [/Devices/usb-ohci/0/LUN#999/] (level 4)
00:00:00.071980   Driver <string>  = "MainStatus" (cb=11)
00:00:00.071980
00:00:00.071980 [/Devices/usb-ohci/0/LUN#999/Config/] (level 5)
00:00:00.071981   First                <integer> = 0x0000000000000000 (0)
00:00:00.071982   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.071982   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.071983   iLedSet              <integer> = 0x0000000000000001 (1)
00:00:00.071984
00:00:00.071984 [/Devices/vga/] (level 2)
00:00:00.071984
00:00:00.071985 [/Devices/vga/0/] (level 3)
00:00:00.071985   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.071986   PCIDeviceNo   <integer> = 0x0000000000000002 (2)
00:00:00.071986   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.071987   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.071988
00:00:00.071988 [/Devices/vga/0/Config/] (level 4)
00:00:00.071989   3DEnabled        <integer> = 0x0000000000000000 (0)
00:00:00.071989   CustomVideoModes <integer> = 0x0000000000000000 (0)
00:00:00.071990   FadeIn           <integer> = 0x0000000000000001 (1)
00:00:00.071990   FadeOut          <integer> = 0x0000000000000001 (1)
00:00:00.071991   HeightReduction  <integer> = 0x0000000000000000 (0)
00:00:00.071992   LogoFile         <string>  = "" (cb=1)
00:00:00.071992   LogoTime         <integer> = 0x0000000000000000 (0)
00:00:00.071993   MonitorCount     <integer> = 0x0000000000000008 (8)
00:00:00.071993   ShowBootMenu     <integer> = 0x0000000000000002 (2)
00:00:00.071994   VRamSize         <integer> = 0x0000000008000000 (134 217 728, 128.0 MiB)
00:00:00.071995
00:00:00.071995 [/Devices/vga/0/LUN#0/] (level 4)
00:00:00.071996   Driver <string>  = "MainDisplay" (cb=12)
00:00:00.071996
00:00:00.071997 [/Devices/vga/0/LUN#0/Config/] (level 5)
00:00:00.071997
00:00:00.071998 [/Devices/vga/0/LUN#999/] (level 4)
00:00:00.071998   Driver <string>  = "MainStatus" (cb=11)
00:00:00.071999
00:00:00.071999 [/Devices/vga/0/LUN#999/Config/] (level 5)
00:00:00.072000   First                <integer> = 0x0000000000000000 (0)
00:00:00.072000   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.072001   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.072002   iLedSet              <integer> = 0x0000000000000000 (0)
00:00:00.072002
00:00:00.072003 [/Devices/virtio-net/] (level 2)
00:00:00.072003
00:00:00.072003 [/EM/] (level 1)
00:00:00.072004   TripleFaultReset <integer> = 0x0000000000000000 (0)
00:00:00.072004
00:00:00.072005 [/GCM/] (level 1)
00:00:00.072005
00:00:00.072005 [/GIM/] (level 1)
00:00:00.072006   Provider <string>  = "None" (cb=5)
00:00:00.072006
00:00:00.072007 [/HM/] (level 1)
00:00:00.072008   64bitEnabled        <integer> = 0x0000000000000000 (0)
00:00:00.072008   EnableLargePages    <integer> = 0x0000000000000000 (0)
00:00:00.072009   EnableNestedPaging  <integer> = 0x0000000000000001 (1)
00:00:00.072009   EnableUX            <integer> = 0x0000000000000001 (1)
00:00:00.072010   EnableVPID          <integer> = 0x0000000000000001 (1)
00:00:00.072010   Exclusive           <integer> = 0x0000000000000001 (1)
00:00:00.072011   HMForced            <integer> = 0x0000000000000001 (1)
00:00:00.072012   IBPBOnVMEntry       <integer> = 0x0000000000000000 (0)
00:00:00.072012   IBPBOnVMExit        <integer> = 0x0000000000000000 (0)
00:00:00.072013   L1DFlushOnSched     <integer> = 0x0000000000000001 (1)
00:00:00.072013   L1DFlushOnVMEntry   <integer> = 0x0000000000000000 (0)
00:00:00.072014   MDSClearOnSched     <integer> = 0x0000000000000001 (1)
00:00:00.072015   MDSClearOnVMEntry   <integer> = 0x0000000000000000 (0)
00:00:00.072015   SpecCtrlByHost      <integer> = 0x0000000000000000 (0)
00:00:00.072016   SvmVirtVmsaveVmload <integer> = 0x0000000000000000 (0)
00:00:00.072016   UseNEMInstead       <integer> = 0x0000000000000000 (0)
00:00:00.072017
00:00:00.072017 [/MM/] (level 1)
00:00:00.072017   CanUseLargerHeap <integer> = 0x0000000000000000 (0)
00:00:00.072018
00:00:00.072018 [/NEM/] (level 1)
00:00:00.072019   Allow64BitGuests  <integer> = 0x0000000000000000 (0)
00:00:00.072019   IBPBOnVMEntry     <integer> = 0x0000000000000000 (0)
00:00:00.072020   IBPBOnVMExit      <integer> = 0x0000000000000000 (0)
00:00:00.072020   L1DFlushOnSched   <integer> = 0x0000000000000001 (1)
00:00:00.072021   L1DFlushOnVMEntry <integer> = 0x0000000000000000 (0)
00:00:00.072021   MDSClearOnSched   <integer> = 0x0000000000000001 (1)
00:00:00.072022   MDSClearOnVMEntry <integer> = 0x0000000000000000 (0)
00:00:00.072023
00:00:00.072023 [/PDM/] (level 1)
00:00:00.072023
00:00:00.072023 [/PDM/AsyncCompletion/] (level 2)
00:00:00.072024
00:00:00.072024 [/PDM/AsyncCompletion/File/] (level 3)
00:00:00.072025
00:00:00.072025 [/PDM/AsyncCompletion/File/BwGroups/] (level 4)
00:00:00.072026
00:00:00.072026 [/PDM/BlkCache/] (level 2)
00:00:00.072027   CacheSize <integer> = 0x0000000000500000 (5 242 880, 5.0 MiB)
00:00:00.072028
00:00:00.072028 [/PDM/Devices/] (level 2)
00:00:00.072029
00:00:00.072029 [/PDM/Drivers/] (level 2)
00:00:00.072029
00:00:00.072030 [/PDM/Drivers/VBoxC/] (level 3)
00:00:00.072030   Path <string>  = "/usr/lib/virtualbox/components/VBoxC" (cb=37)
00:00:00.072031
00:00:00.072031 [/PDM/NetworkShaper/] (level 2)
00:00:00.072031
00:00:00.072032 [/PDM/NetworkShaper/BwGroups/] (level 3)
00:00:00.072032
00:00:00.072033 [/TM/] (level 1)
00:00:00.072033   UTCOffset <integer> = 0x0000000000000000 (0)
00:00:00.072034
00:00:00.072034 [/USB/] (level 1)
00:00:00.072034
00:00:00.072034 [/USB/USBProxy/] (level 2)
00:00:00.072035
00:00:00.072035 [/USB/USBProxy/GlobalConfig/] (level 3)
00:00:00.072036
00:00:00.072036 ********************* End of CFGM dump **********************
00:00:00.072138 HM: HMR3Init: VT-x w/ nested paging and unrestricted guest execution hw support
00:00:00.072184 CPUM: fXStateHostMask=0x7; host XCR0=0x7
00:00:00.072399 CPUM: Matched host CPU INTEL 0x6/0x3a/0x9 Intel_Core7_IvyBridge with CPU DB entry 'Intel Core i5-3570' (INTEL 0x6/0x3a/0x9 Intel_Core7_IvyBridge)
00:00:00.072427 CPUM: MXCSR_MASK=0xffff (host: 0xffff)
00:00:00.072436 CPUM: Microcode revision 0x00000021
00:00:00.072445 CPUM: MSR/CPUID reconciliation insert: 0x0000010b IA32_FLUSH_CMD
00:00:00.072453 CPUM: Enabled MTRR read-write support
00:00:00.072456 CPUM: Enabled fixed-range MTRRs and 16 (virtualized) variable-range MTRRs
00:00:00.073719 PGM: Host paging mode: AMD64+PGE+NX
00:00:00.073726 PGM: PGMPool: cMaxPages=2304 (u64MaxPages=2084)
00:00:00.073729 PGM: pgmR3PoolInit: cMaxPages=0x900 cMaxUsers=0x1200 cMaxPhysExts=0x1200 fCacheEnable=true
00:00:00.074189 PGM: /proc/sys/vm/max_map_count = 1048576 (rc2=VWRN_TRAILING_CHARS); cGuessNeeded=16384
00:00:00.079844 TM: GIP - u32Mode=3 (Invariant) u32UpdateHz=100 u32UpdateIntervalNS=10000000 enmUseTscDelta=2 (Practically Zero) fGetGipCpu=0x1b cCpus=8
00:00:00.079861 TM: GIP - u64CpuHz=3 392 292 995 (0xca324883)  SUPGetCpuHzFromGip => 3 392 292 995
00:00:00.079866 TM: GIP - CPU: iCpuSet=0x0 idCpu=0x0 idApic=0x0 iGipCpu=0x5 i64TSCDelta=0 enmState=3 u64CpuHz=3392292982(*) cErrors=0
00:00:00.079869 TM: GIP - CPU: iCpuSet=0x1 idCpu=0x1 idApic=0x2 iGipCpu=0x7 i64TSCDelta=0 enmState=3 u64CpuHz=3392292892(*) cErrors=0
00:00:00.079878 TM: GIP - CPU: iCpuSet=0x2 idCpu=0x2 idApic=0x4 iGipCpu=0x0 i64TSCDelta=0 enmState=3 u64CpuHz=3392292995(*) cErrors=0
00:00:00.079880 TM: GIP - CPU: iCpuSet=0x3 idCpu=0x3 idApic=0x6 iGipCpu=0x3 i64TSCDelta=0 enmState=3 u64CpuHz=3392292755(*) cErrors=0
00:00:00.079882 TM: GIP - CPU: iCpuSet=0x4 idCpu=0x4 idApic=0x1 iGipCpu=0x4 i64TSCDelta=0 enmState=3 u64CpuHz=3392292741(*) cErrors=0
00:00:00.079884 TM: GIP - CPU: iCpuSet=0x5 idCpu=0x5 idApic=0x3 iGipCpu=0x6 i64TSCDelta=0 enmState=3 u64CpuHz=3392292873(*) cErrors=0
00:00:00.079886 TM: GIP - CPU: iCpuSet=0x6 idCpu=0x6 idApic=0x5 iGipCpu=0x1 i64TSCDelta=0 enmState=3 u64CpuHz=3392285794(*) cErrors=0
00:00:00.079887 TM: GIP - CPU: iCpuSet=0x7 idCpu=0x7 idApic=0x7 iGipCpu=0x2 i64TSCDelta=0 enmState=3 u64CpuHz=3392290916(*) cErrors=0
00:00:00.079897 TM:     cTSCTicksPerSecond=3 392 292 995 (0xca324883) enmTSCMode=1 (VirtTSCEmulated) TSCMultiplier=1
00:00:00.079898 TM: cTSCTicksPerSecondHost=3 392 292 995 (0xca324883)
00:00:00.079899 TM: TSCTiedToExecution=false TSCNotTiedToHalt=false
00:00:00.080488 EMR3Init: fIemExecutesAll=false fGuruOnTripleFault=true
00:00:00.080765 IEM: TargetCpu=CURRENT, Microarch=Intel_Core7_IvyBridge aidxTargetCpuEflFlavour={1,0}
00:00:00.081556 GIM: Using provider 'None' (Implementation version: 0)
00:00:00.081566 GCM: Initialized - Fixer bits: 0x0
00:00:00.094461 AIOMgr: Default manager type is 'Async'
00:00:00.094483 AIOMgr: Default file backend is 'NonBuffered'
00:00:00.094553 BlkCache: Cache successfully initialized. Cache size is 5242880 bytes
00:00:00.094556 BlkCache: Cache commit interval is 10000 ms
00:00:00.094557 BlkCache: Cache commit threshold is 2621440 bytes
00:00:00.095484 PcBios: [SMP] BIOS with 4 CPUs
00:00:00.095498 PcBios: Using the 386+ BIOS image.
00:00:00.095554 PcBios: MPS table at 000e1300
00:00:00.095980 PcBios: fCheckShutdownStatusForSoftReset=true  fClearShutdownStatusOnHardReset=true
00:00:00.096316 SUP: seg #0: R   0x00000000 LB 0x00009000
00:00:00.096320 SUP: seg #1: R X 0x00009000 LB 0x00030000
00:00:00.096322 SUP: seg #2: R   0x00039000 LB 0x0000f000
00:00:00.096324 SUP: seg #3: RW  0x00048000 LB 0x00007500
00:00:00.096441 SUP: Loaded VBoxDDR0.r0 (/usr/lib/virtualbox/VBoxDDR0.r0) at 0xXXXXXXXXXXXXXXXX - ModuleInit at XXXXXXXXXXXXXXXX and ModuleTerm at XXXXXXXXXXXXXXXX
00:00:00.096598 PDM: VirtualBox APIC backend registered
00:00:00.096603 CPUM: SetGuestCpuIdFeature: Enabled xAPIC
00:00:00.097426 IOAPIC: Version=2.0 ChipType=ICH9
00:00:00.097465 PIT: mode=3 count=0x10000 (65536) - 18.20 Hz (ch=0)
00:00:00.097623 VMMDev: cbDefaultBudget: 696 794 965 (29883f55)
00:00:00.099363 Shared Folders service loaded
00:00:00.099715 Guest Control service loaded
00:00:00.128613 VGA: Using the 386+ BIOS image.
00:00:00.129321 DrvVD: Flushes will be ignored
00:00:00.129327 DrvVD: Async flushes will be passed to the disk
00:00:00.129407 VD: VDInit finished with VINF_SUCCESS
00:00:00.129454 VD: Opening the disk took 123304 ns
00:00:00.129476 PIIX3 ATA: LUN#0: disk, PCHS=4161/16/63, total number of sectors 4194304
00:00:00.130847 PIIX3 ATA: LUN#1: no unit
00:00:00.130983 DrvVD: Flushes will be ignored
00:00:00.130986 DrvVD: Async flushes will be passed to the disk
00:00:00.131021 VD: Opening the disk took 32912 ns
00:00:00.131035 PIIX3 ATA: LUN#2: CD/DVD, total number of sectors 16327, passthrough disabled
00:00:00.131075 PIIX3 ATA: LUN#3: no unit
00:00:00.131164 PIIX3 ATA: Ctl#1: finished processing RESET
00:00:00.131176 PIIX3 ATA: Ctl#0: finished processing RESET
00:00:00.131801 AC97: Using codec 'STAC9700'
00:00:00.131845 Audio: Initializing ALSA driver
00:00:00.152925 ALSA: The ALSAAudio plugin for pulse audio is being used (pulse).
00:00:00.152951 Audio: Found 28 devices for driver 'ALSA'
00:00:00.152960 Audio: Device 'Rate Converter Plugin Using Libav/FFmpeg Library':
00:00:00.152960 Audio:   ID              = lavrate
00:00:00.152960 Audio:   Usage           = duplex
00:00:00.152961 Audio:   Flags           = NONE
00:00:00.152961 Audio:   Input channels  = 2
00:00:00.152962 Audio:   Output channels = 2
00:00:00.152963 Audio: Device 'Rate Converter Plugin Using Samplerate Library':
00:00:00.152964 Audio:   ID              = samplerate
00:00:00.152964 Audio:   Usage           = duplex
00:00:00.152964 Audio:   Flags           = NONE
00:00:00.152964 Audio:   Input channels  = 2
00:00:00.152965 Audio:   Output channels = 2
00:00:00.152973 Audio: Device 'Rate Converter Plugin Using Speex Resampler':
00:00:00.152973 Audio:   ID              = speexrate
00:00:00.152974 Audio:   Usage           = duplex
00:00:00.152974 Audio:   Flags           = NONE
00:00:00.152974 Audio:   Input channels  = 2
00:00:00.152974 Audio:   Output channels = 2
00:00:00.152976 Audio: Device 'JACK Audio Connection Kit':
00:00:00.152976 Audio:   ID              = jack
00:00:00.152976 Audio:   Usage           = duplex
00:00:00.152976 Audio:   Flags           = NONE
00:00:00.152977 Audio:   Input channels  = 2
00:00:00.152977 Audio:   Output channels = 2
00:00:00.152978 Audio: Device 'Open Sound System':
00:00:00.152978 Audio:   ID              = oss
00:00:00.152978 Audio:   Usage           = duplex
00:00:00.152979 Audio:   Flags           = NONE
00:00:00.152979 Audio:   Input channels  = 2
00:00:00.152979 Audio:   Output channels = 2
00:00:00.152980 Audio: Device 'PipeWire Sound Server':
00:00:00.152981 Audio:   ID              = pipewire
00:00:00.152981 Audio:   Usage           = duplex
00:00:00.152981 Audio:   Flags           = NONE
00:00:00.152981 Audio:   Input channels  = 2
00:00:00.152982 Audio:   Output channels = 2
00:00:00.152983 Audio: Device 'PulseAudio Sound Server':
00:00:00.152983 Audio:   ID              = pulse
00:00:00.152983 Audio:   Usage           = duplex
00:00:00.152983 Audio:   Flags           = NONE
00:00:00.152983 Audio:   Input channels  = 2
00:00:00.152984 Audio:   Output channels = 2
00:00:00.152985 Audio: Device 'Plugin using Speex DSP (resample, agc, denoise, echo, dereverb)':
00:00:00.152985 Audio:   ID              = speex
00:00:00.152985 Audio:   Usage           = duplex
00:00:00.152986 Audio:   Flags           = NONE
00:00:00.152986 Audio:   Input channels  = 2
00:00:00.152986 Audio:   Output channels = 2
00:00:00.152987 Audio: Device 'Plugin for channel upmix (4,6,8)':
00:00:00.152987 Audio:   ID              = upmix
00:00:00.152988 Audio:   Usage           = duplex
00:00:00.152988 Audio:   Flags           = NONE
00:00:00.152988 Audio:   Input channels  = 2
00:00:00.152988 Audio:   Output channels = 2
00:00:00.152989 Audio: Device 'Plugin for channel downmix (stereo) with a simple spacialization':
00:00:00.152990 Audio:   ID              = vdownmix
00:00:00.152990 Audio:   Usage           = duplex
00:00:00.152990 Audio:   Flags           = NONE
00:00:00.152990 Audio:   Input channels  = 2
00:00:00.152991 Audio:   Output channels = 2
00:00:00.152992 Audio: Device 'Default ALSA Output (currently PipeWire Media Server)':
00:00:00.152992 Audio:   ID              = default
00:00:00.152992 Audio:   Usage           = duplex
00:00:00.152992 Audio:   Flags           = NONE
00:00:00.152993 Audio:   Input channels  = 2
00:00:00.152993 Audio:   Output channels = 2
00:00:00.152994 Audio: Device 'Default Audio Device (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.152994 Audio:   ID              = sysdefault:CARD=PCH
00:00:00.152995 Audio:   Usage           = duplex
00:00:00.152995 Audio:   Flags           = NONE
00:00:00.152995 Audio:   Input channels  = 2
00:00:00.152995 Audio:   Output channels = 2
00:00:00.152996 Audio: Device 'Front output / input (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.152997 Audio:   ID              = front:CARD=PCH,DEV=0
00:00:00.152997 Audio:   Usage           = duplex
00:00:00.152997 Audio:   Flags           = NONE
00:00:00.152997 Audio:   Input channels  = 2
00:00:00.152998 Audio:   Output channels = 2
00:00:00.152999 Audio: Device '2.1 Surround output to Front and Subwoofer speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.152999 Audio:   ID              = surround21:CARD=PCH,DEV=0
00:00:00.152999 Audio:   Usage           = output
00:00:00.152999 Audio:   Flags           = NONE
00:00:00.153000 Audio:   Input channels  = 0
00:00:00.153000 Audio:   Output channels = 2
00:00:00.153003 Audio: Device '4.0 Surround output to Front and Rear speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.153003 Audio:   ID              = surround40:CARD=PCH,DEV=0
00:00:00.153004 Audio:   Usage           = output
00:00:00.153004 Audio:   Flags           = NONE
00:00:00.153004 Audio:   Input channels  = 0
00:00:00.153004 Audio:   Output channels = 2
00:00:00.153005 Audio: Device '4.1 Surround output to Front, Rear and Subwoofer speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.153006 Audio:   ID              = surround41:CARD=PCH,DEV=0
00:00:00.153006 Audio:   Usage           = output
00:00:00.153006 Audio:   Flags           = NONE
00:00:00.153007 Audio:   Input channels  = 0
00:00:00.153007 Audio:   Output channels = 2
00:00:00.153008 Audio: Device '5.0 Surround output to Front, Center and Rear speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.153008 Audio:   ID              = surround50:CARD=PCH,DEV=0
00:00:00.153008 Audio:   Usage           = output
00:00:00.153009 Audio:   Flags           = NONE
00:00:00.153009 Audio:   Input channels  = 0
00:00:00.153009 Audio:   Output channels = 2
00:00:00.153010 Audio: Device '5.1 Surround output to Front, Center, Rear and Subwoofer speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.153011 Audio:   ID              = surround51:CARD=PCH,DEV=0
00:00:00.153011 Audio:   Usage           = output
00:00:00.153011 Audio:   Flags           = NONE
00:00:00.153011 Audio:   Input channels  = 0
00:00:00.153012 Audio:   Output channels = 2
00:00:00.153013 Audio: Device '7.1 Surround output to Front, Center, Side, Rear and Woofer speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.153013 Audio:   ID              = surround71:CARD=PCH,DEV=0
00:00:00.153013 Audio:   Usage           = output
00:00:00.153013 Audio:   Flags           = NONE
00:00:00.153014 Audio:   Input channels  = 0
00:00:00.153014 Audio:   Output channels = 2
00:00:00.153015 Audio: Device 'USB Stream Output (HDA Intel PCH)':
00:00:00.153015 Audio:   ID              = usbstream:CARD=PCH
00:00:00.153015 Audio:   Usage           = duplex
00:00:00.153016 Audio:   Flags           = NONE
00:00:00.153016 Audio:   Input channels  = 2
00:00:00.153016 Audio:   Output channels = 2
00:00:00.153017 Audio: Device 'HDMI Audio Output (HDA NVidia, Smart TV)':
00:00:00.153017 Audio:   ID              = hdmi:CARD=NVidia,DEV=0
00:00:00.153018 Audio:   Usage           = output
00:00:00.153018 Audio:   Flags           = NONE
00:00:00.153018 Audio:   Input channels  = 0
00:00:00.153018 Audio:   Output channels = 2
00:00:00.153019 Audio: Device 'HDMI Audio Output (HDA NVidia, HDMI 1)':
00:00:00.153020 Audio:   ID              = hdmi:CARD=NVidia,DEV=1
00:00:00.153020 Audio:   Usage           = output
00:00:00.153020 Audio:   Flags           = NONE
00:00:00.153020 Audio:   Input channels  = 0
00:00:00.153021 Audio:   Output channels = 2
00:00:00.153022 Audio: Device 'HDMI Audio Output (HDA NVidia, HDMI 2)':
00:00:00.153022 Audio:   ID              = hdmi:CARD=NVidia,DEV=2
00:00:00.153022 Audio:   Usage           = output
00:00:00.153022 Audio:   Flags           = NONE
00:00:00.153023 Audio:   Input channels  = 0
00:00:00.153023 Audio:   Output channels = 2
00:00:00.153024 Audio: Device 'HDMI Audio Output (HDA NVidia, HDMI 3)':
00:00:00.153024 Audio:   ID              = hdmi:CARD=NVidia,DEV=3
00:00:00.153024 Audio:   Usage           = output
00:00:00.153025 Audio:   Flags           = NONE
00:00:00.153025 Audio:   Input channels  = 0
00:00:00.153025 Audio:   Output channels = 2
00:00:00.153026 Audio: Device 'USB Stream Output (HDA NVidia)':
00:00:00.153027 Audio:   ID              = usbstream:CARD=NVidia
00:00:00.153027 Audio:   Usage           = duplex
00:00:00.153027 Audio:   Flags           = NONE
00:00:00.153027 Audio:   Input channels  = 2
00:00:00.153027 Audio:   Output channels = 2
00:00:00.153029 Audio: Device 'Default Audio Device (USB2.0_Camera, USB Audio)':
00:00:00.153029 Audio:   ID              = sysdefault:CARD=USB20Camera
00:00:00.153029 Audio:   Usage           = input
00:00:00.153029 Audio:   Flags           = NONE
00:00:00.153030 Audio:   Input channels  = 2
00:00:00.153030 Audio:   Output channels = 0
00:00:00.153033 Audio: Device 'Front output / input (USB2.0_Camera, USB Audio)':
00:00:00.153033 Audio:   ID              = front:CARD=USB20Camera,DEV=0
00:00:00.153033 Audio:   Usage           = input
00:00:00.153033 Audio:   Flags           = NONE
00:00:00.153034 Audio:   Input channels  = 2
00:00:00.153034 Audio:   Output channels = 0
00:00:00.153035 Audio: Device 'USB Stream Output (USB2.0_Camera)':
00:00:00.153035 Audio:   ID              = usbstream:CARD=USB20Camera
00:00:00.153036 Audio:   Usage           = duplex
00:00:00.153036 Audio:   Flags           = NONE
00:00:00.153036 Audio:   Input channels  = 2
00:00:00.153036 Audio:   Output channels = 2
00:00:00.153086 AC97: Reset
00:00:00.153088 AC97: Mixer reset (EAID=0x809, EACS=0x9)
00:00:00.153090 AC97: Record select to left=mic, right=mic
00:00:00.153092 Audio Mixer: MUTING master volume of 'AC'97 Mixer' -- channel volumes: ff ff ff ff ff ff ff ff ff ff ff ff
00:00:00.153095 Audio Mixer: MUTING sink 'AC'97 Mixer/Line In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.153097 Audio Mixer: MUTING sink 'AC'97 Mixer/Microphone In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.153099 Audio Mixer: MUTING sink 'AC'97 Mixer/PCM Output' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.153102 Audio Mixer: MUTING sink 'AC'97 Mixer/PCM Output' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.153104 Audio Mixer: MUTING sink 'AC'97 Mixer/Line In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.153106 Audio Mixer: MUTING sink 'AC'97 Mixer/Line In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.153108 Audio Mixer: MUTING sink 'AC'97 Mixer/Microphone In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.153694 PGM: The CPU physical address width is 36 bits
00:00:00.153711 PGM: PGMR3InitFinalize: 4 MB PSE mask 0000000fffffffff -> VINF_SUCCESS
00:00:00.153718 TM: TMR3InitFinalize: fTSCModeSwitchAllowed=false
00:00:00.153813 CPUM: Mapped 1.0MiB (1048576 bytes) of RAM using fixed-range MTRRs
00:00:00.153818 CPUM: Mapped 4.0GiB (4294967296 bytes) of RAM using 3 variable-range MTRRs
00:00:00.153965 VMM: Enabled thread-context hooks
00:00:00.153967 VMM: RTThreadPreemptIsPending() can be trusted
00:00:00.153968 VMM: Kernel preemption is possible
00:00:00.154078 HM: Host MSR_IA32_FEATURE_CONTROL = 0x5
00:00:00.154384 HM: fWorldSwitcher=0x30000 (fIbpbOnVmExit=false fIbpbOnVmEntry=false fL1dFlushOnVmEntry=false); fL1dFlushOnSched=true fMdsClearOnVmEntry=false
00:00:00.154390 HM: Using VT-x implementation 3.0
00:00:00.154390 HM: Max resume loops                  = 8192
00:00:00.154390 HM: Host CR0                          = 0x80050033
00:00:00.154391 HM: Host CR4                          = 0x1726f0
00:00:00.154392 HM: Host EFER                         = 0xd01
00:00:00.154392 HM: Host SMM_MONITOR_CTL              = 0x0
00:00:00.154392 HM: Host CORE_CAPABILITIES            = 0x0
00:00:00.154393 HM: Host MEMORY_CTRL                  = 0x0
00:00:00.154393 HM: Host DR6 zero'ed                  = 0xffff0ff0
00:00:00.154394 HM: MSR_IA32_FEATURE_CONTROL          = 0x5
00:00:00.154395 HM:   LOCK
00:00:00.154395 HM:   VMXON
00:00:00.154395 HM: MSR_IA32_VMX_BASIC                = 0xda040000000010
00:00:00.154396 HM:   VMCS id                           = 0x10
00:00:00.154396 HM:   VMCS size                         = 1024 bytes
00:00:00.154397 HM:   VMCS physical address limit       = None
00:00:00.154397 HM:   VMCS memory type                  = Write Back (WB)
00:00:00.154398 HM:   Dual-monitor treatment support    = true
00:00:00.154398 HM:   OUTS & INS instruction-info       = true
00:00:00.154399 HM:   Supports true-capability MSRs     = true
00:00:00.154399 HM:   VM-entry Xcpt error-code optional = false
00:00:00.154399 HM: MSR_IA32_VMX_PINBASED_CTLS        = 0x7f00000016
00:00:00.154400 HM:   EXT_INT_EXIT
00:00:00.154400 HM:   NMI_EXIT
00:00:00.154409 HM:   VIRTUAL_NMI
00:00:00.154410 HM:   PREEMPT_TIMER
00:00:00.154410 HM:   POSTED_INT (must be cleared)
00:00:00.154410 HM: MSR_IA32_VMX_PROCBASED_CTLS       = 0xfff9fffe0401e172
00:00:00.154411 HM:   INT_WINDOW_EXIT
00:00:00.154428 HM:   USE_TSC_OFFSETTING
00:00:00.154428 HM:   HLT_EXIT
00:00:00.154429 HM:   INVLPG_EXIT
00:00:00.154429 HM:   MWAIT_EXIT
00:00:00.154429 HM:   RDPMC_EXIT
00:00:00.154429 HM:   RDTSC_EXIT
00:00:00.154430 HM:   CR3_LOAD_EXIT (must be set)
00:00:00.154430 HM:   CR3_STORE_EXIT (must be set)
00:00:00.154430 HM:   USE_TERTIARY_CTLS (must be cleared)
00:00:00.154431 HM:   CR8_LOAD_EXIT
00:00:00.154431 HM:   CR8_STORE_EXIT
00:00:00.154431 HM:   USE_TPR_SHADOW
00:00:00.154432 HM:   NMI_WINDOW_EXIT
00:00:00.154432 HM:   MOV_DR_EXIT
00:00:00.154432 HM:   UNCOND_IO_EXIT
00:00:00.154432 HM:   USE_IO_BITMAPS
00:00:00.154433 HM:   MONITOR_TRAP_FLAG
00:00:00.154433 HM:   USE_MSR_BITMAPS
00:00:00.154433 HM:   MONITOR_EXIT
00:00:00.154433 HM:   PAUSE_EXIT
00:00:00.154434 HM:   USE_SECONDARY_CTLS
00:00:00.154434 HM: MSR_IA32_VMX_PROCBASED_CTLS2      = 0x8ff00000000
00:00:00.154435 HM:   VIRT_APIC_ACCESS
00:00:00.154435 HM:   EPT
00:00:00.154436 HM:   DESC_TABLE_EXIT
00:00:00.154436 HM:   RDTSCP
00:00:00.154436 HM:   VIRT_X2APIC_MODE
00:00:00.154436 HM:   VPID
00:00:00.154437 HM:   WBINVD_EXIT
00:00:00.154437 HM:   UNRESTRICTED_GUEST
00:00:00.154437 HM:   APIC_REG_VIRT (must be cleared)
00:00:00.154438 HM:   VIRT_INT_DELIVERY (must be cleared)
00:00:00.154438 HM:   PAUSE_LOOP_EXIT (must be cleared)
00:00:00.154438 HM:   RDRAND_EXIT
00:00:00.154438 HM:   INVPCID (must be cleared)
00:00:00.154439 HM:   VMFUNC (must be cleared)
00:00:00.154439 HM:   VMCS_SHADOWING (must be cleared)
00:00:00.154439 HM:   ENCLS_EXIT (must be cleared)
00:00:00.154439 HM:   RDSEED_EXIT (must be cleared)
00:00:00.154440 HM:   PML (must be cleared)
00:00:00.154440 HM:   EPT_XCPT_VE (must be cleared)
00:00:00.154440 HM:   CONCEAL_VMX_FROM_PT (must be cleared)
00:00:00.154441 HM:   XSAVES_XRSTORS (must be cleared)
00:00:00.154441 HM:   PASID_TRANSLATE (must be cleared)
00:00:00.154441 HM:   MODE_BASED_EPT_PERM (must be cleared)
00:00:00.154441 HM:   SPP_EPT (must be cleared)
00:00:00.154442 HM:   PT_EPT (must be cleared)
00:00:00.154442 HM:   TSC_SCALING (must be cleared)
00:00:00.154442 HM:   USER_WAIT_PAUSE (must be cleared)
00:00:00.154443 HM:   PCONFIG (must be cleared)
00:00:00.154443 HM:   ENCLV_EXIT (must be cleared)
00:00:00.154443 HM:   BUS_LOCK_DETECT (must be cleared)
00:00:00.154443 HM:   INSTR_TIMEOUT (must be cleared)
00:00:00.154444 HM: MSR_IA32_VMX_ENTRY_CTLS           = 0xffff000011ff
00:00:00.154444 HM:   LOAD_DEBUG (must be set)
00:00:00.154445 HM:   IA32E_MODE_GUEST
00:00:00.154445 HM:   ENTRY_TO_SMM
00:00:00.154445 HM:   DEACTIVATE_DUAL_MON
00:00:00.154445 HM:   LOAD_PERF_MSR
00:00:00.154446 HM:   LOAD_PAT_MSR
00:00:00.154446 HM:   LOAD_EFER_MSR
00:00:00.154446 HM:   LOAD_BNDCFGS_MSR (must be cleared)
00:00:00.154446 HM:   CONCEAL_VMX_FROM_PT (must be cleared)
00:00:00.154447 HM:   LOAD_RTIT_CTL_MSR (must be cleared)
00:00:00.154447 HM:   LOAD_UINV (must be cleared)
00:00:00.154447 HM:   LOAD_CET_STATE (must be cleared)
00:00:00.154448 HM:   LOAD_LBR_CTL_MSR (must be cleared)
00:00:00.154448 HM:   LOAD_PKRS_MSR (must be cleared)
00:00:00.154448 HM: MSR_IA32_VMX_EXIT_CTLS            = 0x7fffff00036dff
00:00:00.154449 HM:   SAVE_DEBUG (must be set)
00:00:00.154449 HM:   HOST_ADDR_SPACE_SIZE
00:00:00.154450 HM:   LOAD_PERF_MSR
00:00:00.154450 HM:   ACK_EXT_INT
00:00:00.154450 HM:   SAVE_PAT_MSR
00:00:00.154450 HM:   LOAD_PAT_MSR
00:00:00.154450 HM:   SAVE_EFER_MSR
00:00:00.154451 HM:   LOAD_EFER_MSR
00:00:00.154451 HM:   SAVE_PREEMPT_TIMER
00:00:00.154451 HM:   CLEAR_BNDCFGS_MSR (must be cleared)
00:00:00.154452 HM:   CONCEAL_VMX_FROM_PT (must be cleared)
00:00:00.154452 HM:   CLEAR_RTIT_CTL_MSR (must be cleared)
00:00:00.154452 HM:   CLEAR_LBR_CTL_MSR (must be cleared)
00:00:00.154453 HM:   CLEAR_UINV (must be cleared)
00:00:00.154453 HM:   LOAD_CET_STATE (must be cleared)
00:00:00.154453 HM:   LOAD_PKRS_MSR (must be cleared)
00:00:00.154453 HM:   SAVE_PERF_MSR (must be cleared)
00:00:00.154454 HM: MSR_IA32_VMX_TRUE_PINBASED_CTLS   = 0x7f00000016
00:00:00.154454 HM: MSR_IA32_VMX_TRUE_PROCBASED_CTLS  = 0xfff9fffe04006172
00:00:00.154455 HM: MSR_IA32_VMX_TRUE_ENTRY_CTLS      = 0xffff000011fb
00:00:00.154456 HM: MSR_IA32_VMX_TRUE_EXIT_CTLS       = 0x7fffff00036dfb
00:00:00.154456 HM: MSR_IA32_VMX_MISC                 = 0x100401e5
00:00:00.154457 HM:   PREEMPT_TIMER_TSC                 = 0x5
00:00:00.154457 HM:   EXIT_SAVE_EFER_LMA                = true
00:00:00.154458 HM:   ACTIVITY_STATES                   = 0x7 ( HLT SHUTDOWN SIPI_WAIT )
00:00:00.154458 HM:   INTEL_PT                          = false
00:00:00.154458 HM:   SMM_READ_SMBASE_MSR               = false
00:00:00.154459 HM:   CR3_TARGET                        = 0x4
00:00:00.154459 HM:   MAX_MSR                           = 0x0 ( 512 )
00:00:00.154460 HM:   VMXOFF_BLOCK_SMI                  = true
00:00:00.154460 HM:   VMWRITE_ALL                       = false
00:00:00.154461 HM:   ENTRY_INJECT_SOFT_INT             = 0x0
00:00:00.154461 HM:   MSEG_ID                           = 0x0
00:00:00.154461 HM: MSR_IA32_VMX_VMCS_ENUM            = 0x2a
00:00:00.154462 HM:   HIGHEST_IDX                       = 0x15
00:00:00.154462 HM: MSR_IA32_VMX_EPT_VPID_CAP         = 0xf0106114141
00:00:00.154463 HM:   RWX_X_ONLY
00:00:00.154463 HM:   PAGE_WALK_LENGTH_4
00:00:00.154463 HM:   MEMTYPE_UC
00:00:00.154463 HM:   MEMTYPE_WB
00:00:00.154464 HM:   PDE_2M
00:00:00.154464 HM:   INVEPT
00:00:00.154464 HM:   INVEPT_SINGLE_CONTEXT
00:00:00.154465 HM:   INVEPT_ALL_CONTEXTS
00:00:00.154465 HM:   INVVPID
00:00:00.154465 HM:   INVVPID_INDIV_ADDR
00:00:00.154465 HM:   INVVPID_SINGLE_CONTEXT
00:00:00.154466 HM:   INVVPID_ALL_CONTEXTS
00:00:00.154466 HM:   INVVPID_SINGLE_CONTEXT_RETAIN_GLOBALS
00:00:00.154466 HM: MSR_IA32_VMX_CR0_FIXED0           = 0x80000021
00:00:00.154467 HM: MSR_IA32_VMX_CR0_FIXED1           = 0xffffffff
00:00:00.154467 HM: MSR_IA32_VMX_CR4_FIXED0           = 0x2000
00:00:00.154468 HM: MSR_IA32_VMX_CR4_FIXED1           = 0x1767ff
00:00:00.154468 HM: Guest support: 32-bit only
00:00:00.154473 HM: Supports VMCS EFER fields         = true
00:00:00.154474 HM: Enabled VMX
00:00:00.154476 HM: Enabled nested paging
00:00:00.154477 HM:   EPT flush type                  = Single context
00:00:00.154477 HM: Enabled unrestricted guest execution
00:00:00.154477 HM: Enabled VPID
00:00:00.154478 HM:   VPID flush type                 = Single context
00:00:00.154478 HM: Enabled VMX-preemption timer (cPreemptTimerShift=5)
00:00:00.154478 HM: VT-x/AMD-V init method: Global
00:00:00.154479 HM: VT-x/AMD-V enable method: Host API
00:00:00.154480 EM: Exit history optimizations: enabled=true enabled-r0=true enabled-r0-no-preemption=false
00:00:00.154498 PcBios: ATA LUN#0 LCHS=520/128/63
00:00:00.154504 APIC: fPostedIntrsEnabled=false fVirtApicRegsEnabled=false fSupportsTscDeadline=false
00:00:00.154507 TMR3UtcNow: nsNow=1 783 450 263 355 401 000 nsPrev=0 -> cNsDelta=1 783 450 263 355 401 000 (offLag=0 offVirtualSync=0 offVirtualSyncGivenUp=0, NowAgain=1 783 450 263 355 401 000)
00:00:00.154535 VMM: fUsePeriodicPreemptionTimers=false
00:00:00.154661 CPUM: Logical host processors: 8 present, 8 max, 8 online, online mask: 00000000000000ff
00:00:00.154755 CPUM: Physical host cores: 4
00:00:00.154756 ************************ CPUID dump *************************
00:00:00.154941          Raw Standard CPUID Leaves
00:00:00.154942      Leaf/sub-leaf  eax      ebx      ecx      edx
00:00:00.154942 Gst: 00000000/0000  0000000d 756e6547 6c65746e 49656e69
00:00:00.154943 Hst:                0000000d 756e6547 6c65746e 49656e69
00:00:00.154944 Gst: 00000001/0000  000306a9 00040800 769a2203 178bfbbf
00:00:00.154945 Hst:                000306a9 03100800 7fbae3ff bfebfbff
00:00:00.154946 Gst: 00000002/0000  76035a01 00f0b2ff 00000000 00ca0000
00:00:00.154947 Hst:                76035a01 00f0b2ff 00000000 00ca0000
00:00:00.154947 Gst: 00000003/0000  00000000 00000000 00000000 00000000
00:00:00.154948 Hst:                00000000 00000000 00000000 00000000
00:00:00.154949 Gst: 00000004/0000  0c000121 01c0003f 0000003f 00000000
00:00:00.154949 Hst:                1c004121 01c0003f 0000003f 00000000
00:00:00.154950 Gst: 00000004/0001  0c000122 01c0003f 0000003f 00000000
00:00:00.154951 Hst:                1c004122 01c0003f 0000003f 00000000
00:00:00.154951 Gst: 00000004/0002  0c000143 01c0003f 000001ff 00000000
00:00:00.154952 Hst:                1c004143 01c0003f 000001ff 00000000
00:00:00.154953 Gst: 00000004/0003  0c000163 03c0003f 00001fff 00000006
00:00:00.154954 Hst:                1c03c163 03c0003f 00001fff 00000006
00:00:00.154954 Gst: 00000004/0004  0c000000 00000000 00000000 00000000
00:00:00.154955 Hst:                00000000 00000000 00000000 00000000
00:00:00.154955 Gst: 00000005/0000  00000000 00000000 00000000 00000000
00:00:00.154956 Hst:                00000040 00000040 00000003 00001120
00:00:00.154957 Gst: 00000006/0000  00000004 00000000 00000000 00000000
00:00:00.154957 Hst:                00000077 00000002 00000009 00000000
00:00:00.154958 Gst: 00000007/0000  00000000 00000001 00000000 10000400
00:00:00.154958 Hst:                00000000 00000281 00000000 9c000400
00:00:00.154959 Gst: 00000007/0001  00000000 00000000 00000000 00000000
00:00:00.154960 Hst:                00000000 00000000 00000000 00000000
00:00:00.154960 Gst: 00000007/0002  00000000 00000000 00000000 00000000
00:00:00.154961 Hst:                00000000 00000000 00000000 00000000
00:00:00.154961 Gst: 00000008/0000  00000000 00000000 00000000 00000000
00:00:00.154962 Hst:                00000000 00000000 00000000 00000000
00:00:00.154962 Gst: 00000009/0000  00000000 00000000 00000000 00000000
00:00:00.154963 Hst:                00000000 00000000 00000000 00000000
00:00:00.154963 Gst: 0000000a/0000  00000000 00000000 00000000 00000000
00:00:00.154964 Hst:                07300403 00000000 00000000 00000603
00:00:00.154964 Gst: 0000000b/0000  00000000 00000001 00000100 00000000
00:00:00.154965 Hst:                00000001 00000002 00000100 00000003
00:00:00.154966 Gst: 0000000b/0001  00000002 00000004 00000201 00000000
00:00:00.154966 Hst:                00000004 00000008 00000201 00000003
00:00:00.154967 Gst: 0000000b/0002  00000000 00000000 00000002 00000000
00:00:00.154967 Hst:                00000000 00000000 00000002 00000003
00:00:00.154968 Gst: 0000000c/0000  00000000 00000000 00000000 00000000
00:00:00.154968 Hst:                00000000 00000000 00000000 00000000
00:00:00.154969 Gst: 0000000d/0000  00000007 00000340 00000340 00000000
00:00:00.154969 Hst:                00000007 00000340 00000340 00000000
00:00:00.154970 Gst: 0000000d/0001  00000000 00000000 00000000 00000000
00:00:00.154971 Hst:                00000001 00000000 00000000 00000000
00:00:00.154971 Gst: 0000000d/0002  00000100 00000240 00000000 00000000
00:00:00.154972 Hst:                00000100 00000240 00000000 00000000
00:00:00.154972 Gst: 0000000d/0003  00000000 00000000 00000000 00000000
00:00:00.154973 Hst:                00000000 00000000 00000000 00000000
00:00:00.154973                                Name: GenuineIntel
00:00:00.154974                            Supports: 0x00000000-0x0000000d
00:00:00.154975                              Family:  6 	Extended: 0 	Effective: 6
00:00:00.154976                               Model: 10 	Extended: 3 	Effective: 58
00:00:00.154977                            Stepping: 9
00:00:00.154978                                Type: 0 (primary)
00:00:00.154979                             APIC ID: 0x00
00:00:00.154979                        Logical CPUs: 4
00:00:00.154980                        CLFLUSH Size: 8
00:00:00.154981                            Brand ID: 0x00
00:00:00.154982 Features
00:00:00.154982   Mnemonic - Description                                  = Guest (Host)
00:00:00.154988   FPU - x87 FPU on Chip                                   = 1 (1)
00:00:00.154989   VME - Virtual 8086 Mode Enhancements                    = 1 (1)
00:00:00.154990   DE - Debugging extensions                               = 1 (1)
00:00:00.154991   PSE - Page Size Extension                               = 1 (1)
00:00:00.154992   TSC - Time Stamp Counter                                = 1 (1)
00:00:00.154993   MSR - Model Specific Registers                          = 1 (1)
00:00:00.154993   PAE - Physical Address Extension                        = 0 (1)
00:00:00.154994   MCE - Machine Check Exception                           = 1 (1)
00:00:00.154995   CX8 - CMPXCHG8B instruction                             = 1 (1)
00:00:00.154996   APIC - APIC On-Chip                                     = 1 (1)
00:00:00.154997   SEP - SYSENTER and SYSEXIT Present                      = 1 (1)
00:00:00.154998   MTRR - Memory Type Range Registers                      = 1 (1)
00:00:00.154998   PGE - PTE Global Bit                                    = 1 (1)
00:00:00.154999   MCA - Machine Check Architecture                        = 1 (1)
00:00:00.155000   CMOV - Conditional Move instructions                    = 1 (1)
00:00:00.155001   PAT - Page Attribute Table                              = 1 (1)
00:00:00.155002   PSE-36 - 36-bit Page Size Extension                     = 1 (1)
00:00:00.155002   PSN - Processor Serial Number                           = 0 (0)
00:00:00.155003   CLFSH - CLFLUSH instruction                             = 1 (1)
00:00:00.155004   DS - Debug Store                                        = 0 (1)
00:00:00.155005   ACPI - Thermal Mon. & Soft. Clock Ctrl.                 = 0 (1)
00:00:00.155006   MMX - Intel MMX Technology                              = 1 (1)
00:00:00.155006   FXSR - FXSAVE and FXRSTOR instructions                  = 1 (1)
00:00:00.155007   SSE - SSE support                                       = 1 (1)
00:00:00.155008   SSE2 - SSE2 support                                     = 1 (1)
00:00:00.155009   SS - Self Snoop                                         = 0 (1)
00:00:00.155010   HTT - Hyper-Threading Technology                        = 1 (1)
00:00:00.155011   TM - Therm. Monitor                                     = 0 (1)
00:00:00.155012   PBE - Pending Break Enabled                             = 0 (1)
00:00:00.155013   SSE3 - SSE3 support                                     = 1 (1)
00:00:00.155014   PCLMUL - PCLMULQDQ support (for AES-GCM)                = 1 (1)
00:00:00.155014   DTES64 - DS Area 64-bit Layout                          = 0 (1)
00:00:00.155015   MONITOR - MONITOR/MWAIT instructions                    = 0 (1)
00:00:00.155016   CPL-DS - CPL Qualified Debug Store                      = 0 (1)
00:00:00.155017   VMX - Virtual Machine Extensions                        = 0 (1)
00:00:00.155017   SMX - Safer Mode Extensions                             = 0 (1)
00:00:00.155018   EST - Enhanced SpeedStep Technology                     = 0 (1)
00:00:00.155019   TM2 - Terminal Monitor 2                                = 0 (1)
00:00:00.155020   SSSE3 - Supplemental Streaming SIMD Extensions 3        = 1 (1)
00:00:00.155021   CNTX-ID - L1 Context ID                                 = 0 (0)
00:00:00.155021   SDBG - Silicon Debug interface                          = 0 (0)
00:00:00.155022   FMA - Fused Multiply Add extensions                     = 0 (0)
00:00:00.155023   CX16 - CMPXCHG16B instruction                           = 1 (1)
00:00:00.155024   TPRUPDATE - xTPR Update Control                         = 0 (1)
00:00:00.155025   PDCM - Perf/Debug Capability MSR                        = 0 (1)
00:00:00.155025   PCID - Process Context Identifiers                      = 1 (1)
00:00:00.155026   DCA - Direct Cache Access                               = 0 (0)
00:00:00.155027   SSE4_1 - SSE4_1 support                                 = 1 (1)
00:00:00.155028   SSE4_2 - SSE4_2 support                                 = 1 (1)
00:00:00.155029   X2APIC - x2APIC support                                 = 0 (1)
00:00:00.155029   MOVBE - MOVBE instruction                               = 0 (0)
00:00:00.155030   POPCNT - POPCNT instruction                             = 1 (1)
00:00:00.155031   TSCDEADL - Time Stamp Counter Deadline                  = 0 (1)
00:00:00.155032   AES - AES instructions                                  = 1 (1)
00:00:00.155033   XSAVE - XSAVE instruction                               = 1 (1)
00:00:00.155034   OSXSAVE - OSXSAVE instruction                           = 0 (1)
00:00:00.155034   AVX - AVX support                                       = 1 (1)
00:00:00.155035   F16C - 16-bit floating point conversion instructions    = 1 (1)
00:00:00.155036   RDRAND - RDRAND instruction                             = 1 (1)
00:00:00.155037   HVP - Hypervisor Present (we're a guest)                = 0 (0)
00:00:00.155037 Structured Extended Feature Flags Enumeration (leaf 7):
00:00:00.155038 Sub-leaf 0
00:00:00.155038   Mnemonic - Description                                  = Guest (Host)
00:00:00.155039   FSGSBASE - RDFSBASE/RDGSBASE/WRFSBASE/WRGSBASE instr.   = 1 (1)
00:00:00.155040   TSCADJUST - Supports MSR_IA32_TSC_ADJUST                = 0 (0)
00:00:00.155040   SGX - Supports Software Guard Extensions                = 0 (0)
00:00:00.155041   BMI1 - Advanced Bit Manipulation extension 1            = 0 (0)
00:00:00.155042   HLE - Hardware Lock Elision                             = 0 (0)
00:00:00.155043   AVX2 - Advanced Vector Extensions 2                     = 0 (0)
00:00:00.155043   FDP_EXCPTN_ONLY - FPU DP only updated on exceptions     = 0 (0)
00:00:00.155044   SMEP - Supervisor Mode Execution Prevention             = 0 (1)
00:00:00.155044   BMI2 - Advanced Bit Manipulation extension 2            = 0 (0)
00:00:00.155045   ERMS - Enhanced REP MOVSB/STOSB instructions            = 0 (1)
00:00:00.155046   INVPCID - INVPCID instruction                           = 0 (0)
00:00:00.155046   RTM - Restricted Transactional Memory                   = 0 (0)
00:00:00.155047   PQM - Platform Quality of Service Monitoring            = 0 (0)
00:00:00.155048   DEPFPU_CS_DS - Deprecates FPU CS, FPU DS values if set  = 0 (0)
00:00:00.155048   MPE - Intel Memory Protection Extensions                = 0 (0)
00:00:00.155049   PQE - Platform Quality of Service Enforcement           = 0 (0)
00:00:00.155050   AVX512F - AVX512 Foundation instructions                = 0 (0)
00:00:00.155050   AVX512DQ - Supports the AVX512DQ instructions           = 0 (0)
00:00:00.155051   RDSEED - RDSEED instruction                             = 0 (0)
00:00:00.155052   ADX - ADCX/ADOX instructions                            = 0 (0)
00:00:00.155052   SMAP - Supervisor Mode Access Prevention                = 0 (0)
00:00:00.155053   AVX512_IFMA - Supports the AVX512_IFMA instructions     = 0 (0)
00:00:00.155054   CLFLUSHOPT - CLFLUSHOPT (Cache Line Flush) instruction  = 0 (0)
00:00:00.155054   CLWB - CLWB instruction                                 = 0 (0)
00:00:00.155055   INTEL_PT - Intel Processor Trace                        = 0 (0)
00:00:00.155056   AVX512PF - AVX512 Prefetch instructions                 = 0 (0)
00:00:00.155056   AVX512ER - AVX512 Exponential & Reciprocal instructions = 0 (0)
00:00:00.155057   AVX512CD - AVX512 Conflict Detection instructions       = 0 (0)
00:00:00.155057   SHA - Secure Hash Algorithm extensions                  = 0 (0)
00:00:00.155058   AVX512BW - Supports the AVX512BW instructions           = 0 (0)
00:00:00.155059   AVX512VL - Supports the AVX512VL instructions           = 0 (0)
00:00:00.155059   PREFETCHWT1 - PREFETCHWT1 instruction                   = 0 (0)
00:00:00.155060   AVX512_VBMI - Supports the AVX512_VBMI instructions     = 0 (0)
00:00:00.155061   UMIP - User mode insturction prevention                 = 0 (0)
00:00:00.155061   PKU - Protection Key for Usermode pages                 = 0 (0)
00:00:00.155062   OSPKE - CR4.PKU mirror                                  = 0 (0)
00:00:00.155063   WAITPKG - TPAUSE, UMONITOR & UMWAIT support             = 0 (0)
00:00:00.155064   AVX512_VBMI2 - Supports the AVX512_VBMI2 instructions   = 0 (0)
00:00:00.155064   CET_SS - CET shadow stack support                       = 0 (0)
00:00:00.155065   GFNI - Supports the GFNI instruction set                = 0 (0)
00:00:00.155065   VAES - Supports the VEX encoded AES instruction set     = 0 (0)
00:00:00.155066   VPCLMULQDQ - Supports the VPCLMULQDQ instruction        = 0 (0)
00:00:00.155067   AVX512_VNNI - Supports the AVX512_VNNI instructions     = 0 (0)
00:00:00.155067   AVX512_BITALG - Supports the AVX512_BITALG instructions = 0 (0)
00:00:00.155068   TME_EN - Supports 4 IA32_TME_ MSRs                      = 0 (0)
00:00:00.155068   AVX512_VPOPCNTDQ - Supports the AVX512_VPOPCNTDQ instructions = 0 (0)
00:00:00.155069   LA57 - 57-bit linear addresses                          = 0 (0)
00:00:00.155070   MAWAU - Value used by BNDLDX & BNDSTX                   = 0x0 (0x0)
00:00:00.155071   RDPID - Read processor ID support                       = 0 (0)
00:00:00.155071   KEY_LOCKER - Supports Key Locker                        = 0 (0)
00:00:00.155072   BUS_LOCK_DETECT - Supports OS bus-lock detection        = 0 (0)
00:00:00.155073   CLDEMOTE - Supports cache line demote                   = 0 (0)
00:00:00.155073   MOVDIRI - Supports the MOVDIRI instruction              = 0 (0)
00:00:00.155074   MOVDIRI64B - Supports the MOVDIRI64B instruction        = 0 (0)
00:00:00.155075   ENQCMD - Supports the Eqnqueue Stores                   = 0 (0)
00:00:00.155075   SGX_LC - Supports SGX Launch Configuration              = 0 (0)
00:00:00.155076   PKS - Supports protection keys for supervisor pages     = 0 (0)
00:00:00.155077   SGX_KEYS - Supports Attestation Service for Intel SGX   = 0 (0)
00:00:00.155077   AVX512_4VNNIW - Supports the AVX512_4VNNIW instructions = 0 (0)
00:00:00.155078   AVX512_4FMAPS - Supports the AVX512_4FMAPS instructions = 0 (0)
00:00:00.155078   FAST_SHORT_REP_MOVSB - Supports fast short REP MOVSB    = 0 (0)
00:00:00.155079   UINTR - Supports user interrupts                        = 0 (0)
00:00:00.155080   AVX512_VP2INTERSECT - Supports the AVX512_VP2INTERSECT instr. = 0 (0)
00:00:00.155080   MCU_OPT_CTRL - Supports IA32_MCU_OPT_CTRL               = 0 (0)
00:00:00.155081   MD_CLEAR - Supports MDS related buffer clearing         = 1 (1)
00:00:00.155081   RTM_ALWAYS_ABORT - XBEGIN always aborts and does fallback = 0 (0)
00:00:00.155082   RTM_FORCE_ABORT - Supports IA32_TSX_FORCE_ABORT         = 0 (0)
00:00:00.155082   SERIALIZE - Supports the SERIALIZE instruction          = 0 (0)
00:00:00.155083   HYBRID - Identifiers the CPU as a hybrid part           = 0 (0)
00:00:00.155084   TSXLDTRK - Supports susp/resume of TSX ld addr tracking = 0 (0)
00:00:00.155084   PCONFIG - Supports the PCONFIG instruction              = 0 (0)
00:00:00.155085   ARCH_LBRS - Supports architectural LBRs                 = 0 (0)
00:00:00.155085   CET_IBT - Supports indirect branch tracking w/ CET      = 0 (0)
00:00:00.155086   AMX_BF16 - Supports tile comp. ops on bfloat16 number   = 0 (0)
00:00:00.155086   AVX512_FP16 - Supports the FP16 data type with AVX512   = 0 (0)
00:00:00.155087   AMX_TILE - Supports the tile architecture               = 0 (0)
00:00:00.155088   AMX_INT8 - Supports tile comp. ops on 8-bit integers    = 0 (0)
00:00:00.155088   IBRS_IBPB - IA32_SPEC_CTRL.IBRS and IA32_PRED_CMD.IBPB  = 0 (1)
00:00:00.155089   STIBP - Supports IA32_SPEC_CTRL.STIBP                   = 0 (1)
00:00:00.155089   FLUSH_CMD - Supports IA32_FLUSH_CMD                     = 1 (1)
00:00:00.155090   ARCHCAP - Supports IA32_ARCH_CAP                        = 0 (0)
00:00:00.155091   CORECAP - Supports IA32_CORE_CAP                        = 0 (0)
00:00:00.155091   SSBD - Supports IA32_SPEC_CTRL.SSBD                     = 0 (1)
00:00:00.155092  Sub-leaf 2
00:00:00.155093   Mnemonic - Description                                  = Guest (Host)
00:00:00.155093   PSFD - Supports IA32_SPEC_CTRL[7] (PSFD)                = 0 (0)
00:00:00.155094   IPRED_CTRL - Supports IA32_SPEC_CTRL[4:3] (IPRED_DIS)   = 0 (0)
00:00:00.155094   RRSBA_CTRL - Supports IA32_SPEC_CTRL[6:5] (RRSBA_DIS)   = 0 (0)
00:00:00.155095   DDPD_U - Supports IA32_SPEC_CTRL[8] (DDPD_U)            = 0 (0)
00:00:00.155096   BHI_CTRL - Supports IA32_SPEC_CTRL[10] (BHI_DIS_S)      = 0 (0)
00:00:00.155096   MCDT_NO - No MXCSR Config Dependent Timing issues       = 0 (0)
00:00:00.155097   UC_LOCK_DIS - Supports UC-lock disable and causing #AC  = 0 (0)
00:00:00.155097   MONITOR_MITG_NO - No MONITOR/UMONITOR power issues      = 0 (0)
00:00:00.155098 Processor Extended State Enumeration (leaf 0xd):
00:00:00.155098    XSAVE area cur/max size by XCR0, Guest: 0x340/0x340
00:00:00.155099    XSAVE area cur/max size by XCR0,  Host: 0x340/0x340
00:00:00.155100                    Valid XCR0 bits, Guest: 0x00000000`00000007 ( x87 SSE YMM_Hi128 )
00:00:00.155101                    Valid XCR0 bits,  Host: 0x00000000`00000007 ( x87 SSE YMM_Hi128 )
00:00:00.155103                     XSAVE features, Guest
00:00:00.155103                     XSAVE features,  Host XSAVEOPT
00:00:00.155104       XSAVE area cur size XCR0|XSS, Guest: 0x0
00:00:00.155105       XSAVE area cur size XCR0|XSS,  Host: 0x0
00:00:00.155105                Valid IA32_XSS bits, Guest: 0x00000000`00000000
00:00:00.155106                Valid IA32_XSS bits,  Host: 0x00000000`00000000
00:00:00.155107   State #2, Guest: off=0x0240, cb=0x0100 IA32_XSS-bit -- YMM_Hi128
00:00:00.155108   State #2,  Host:  off=0x0240, cb=0x0100 IA32_XSS-bit -- YMM_Hi128
00:00:00.155112          Raw Extended CPUID Leaves
00:00:00.155112      Leaf/sub-leaf  eax      ebx      ecx      edx
00:00:00.155112 Gst: 80000000/0000  80000008 00000000 00000000 00000000
00:00:00.155113 Hst:                80000008 00000000 00000000 00000000
00:00:00.155113 Gst: 80000001/0000  00000000 00000000 00000001 08000800
00:00:00.155114 Hst:                00000000 00000000 00000001 28100800
00:00:00.155115 Gst: 80000002/0000  20202020 20202020 65746e49 2952286c
00:00:00.155116 Hst:                20202020 20202020 65746e49 2952286c
00:00:00.155117 Gst: 80000003/0000  726f4320 4d542865 37692029 3737332d
00:00:00.155117 Hst:                726f4320 4d542865 37692029 3737332d
00:00:00.155118 Gst: 80000004/0000  50432030 20402055 30342e33 007a4847
00:00:00.155119 Hst:                50432030 20402055 30342e33 007a4847
00:00:00.155120 Gst: 80000005/0000  00000000 00000000 00000000 00000000
00:00:00.155120 Hst:                00000000 00000000 00000000 00000000
00:00:00.155121 Gst: 80000006/0000  00000000 00000000 01006040 00000000
00:00:00.155121 Hst:                00000000 00000000 01006040 00000000
00:00:00.155122 Gst: 80000007/0000  00000000 00000000 00000000 00000100
00:00:00.155123 Hst:                00000000 00000000 00000000 00000100
00:00:00.155123 Gst: 80000008/0000  00003024 00000000 00000000 00000000
00:00:00.155124 Hst:                00003024 00000000 00000000 00000000
00:00:00.155124 Ext Name:
00:00:00.155125 Ext Supports:                    0x80000000-0x80000008
00:00:00.155125 Family:                          0  	Extended: 0 	Effective: 0
00:00:00.155126 Model:                           0  	Extended: 0 	Effective: 0
00:00:00.155126 Stepping:                        0
00:00:00.155126 Brand ID:                        0x000
00:00:00.155127 Ext Features
00:00:00.155127   Mnemonic - Description                                  = Guest (Host)
00:00:00.155128   FPU - x87 FPU on Chip                                   = 0 (0)
00:00:00.155129   VME - Virtual 8086 Mode Enhancements                    = 0 (0)
00:00:00.155130   DE - Debugging extensions                               = 0 (0)
00:00:00.155131   PSE - Page Size Extension                               = 0 (0)
00:00:00.155132   TSC - Time Stamp Counter                                = 0 (0)
00:00:00.155132   MSR - K86 Model Specific Registers                      = 0 (0)
00:00:00.155133   PAE - Physical Address Extension                        = 0 (0)
00:00:00.155134   MCE - Machine Check Exception                           = 0 (0)
00:00:00.155135   CX8 - CMPXCHG8B instruction                             = 0 (0)
00:00:00.155136   APIC - APIC On-Chip                                     = 0 (0)
00:00:00.155137   SEP - SYSCALL/SYSRET                                    = 1 (1)
00:00:00.155137   MTRR - Memory Type Range Registers                      = 0 (0)
00:00:00.155138   PGE - PTE Global Bit                                    = 0 (0)
00:00:00.155139   MCA - Machine Check Architecture                        = 0 (0)
00:00:00.155140   CMOV - Conditional Move instructions                    = 0 (0)
00:00:00.155141   PAT - Page Attribute Table                              = 0 (0)
00:00:00.155141   PSE-36 - 36-bit Page Size Extension                     = 0 (0)
00:00:00.155142   NX - No-Execute/Execute-Disable                         = 0 (1)
00:00:00.155143   AXMMX - AMD Extensions to MMX instructions              = 0 (0)
00:00:00.155144   MMX - Intel MMX Technology                              = 0 (0)
00:00:00.155145   FXSR - FXSAVE and FXRSTOR Instructions                  = 0 (0)
00:00:00.155145   FFXSR - AMD fast FXSAVE and FXRSTOR instructions        = 0 (0)
00:00:00.155146   Page1GB - 1 GB large page                               = 0 (0)
00:00:00.155147   RDTSCP - RDTSCP instruction                             = 1 (1)
00:00:00.155148   LM - AMD64 Long Mode                                    = 0 (1)
00:00:00.155149   3DNOWEXT - AMD Extensions to 3DNow                      = 0 (0)
00:00:00.155149   3DNOW - AMD 3DNow                                       = 0 (0)
00:00:00.155150   LahfSahf - LAHF/SAHF support in 64-bit mode             = 1 (1)
00:00:00.155151   CmpLegacy - Core multi-processing legacy mode           = 0 (0)
00:00:00.155152   SVM - AMD Secure Virtual Machine extensions             = 0 (0)
00:00:00.155152   EXTAPIC - AMD Extended APIC registers                   = 0 (0)
00:00:00.155153   CR8L - AMD LOCK MOV CR0 means MOV CR8                   = 0 (0)
00:00:00.155154   ABM - AMD Advanced Bit Manipulation                     = 0 (0)
00:00:00.155154   SSE4A - SSE4A instructions                              = 0 (0)
00:00:00.155155   MISALIGNSSE - AMD Misaligned SSE mode                   = 0 (0)
00:00:00.155156   3DNOWPRF - AMD PREFETCH and PREFETCHW instructions      = 0 (0)
00:00:00.155156   OSVW - AMD OS Visible Workaround                        = 0 (0)
00:00:00.155157   IBS - Instruct Based Sampling                           = 0 (0)
00:00:00.155158   XOP - Extended Operation support                        = 0 (0)
00:00:00.155159   SKINIT - SKINIT, STGI, and DEV support                  = 0 (0)
00:00:00.155159   WDT - AMD Watchdog Timer support                        = 0 (0)
00:00:00.155160   LWP - Lightweight Profiling support                     = 0 (0)
00:00:00.155161   FMA4 - Four operand FMA instruction support             = 0 (0)
00:00:00.155161   TCE - Translation Cache Extension support               = 0 (0)
00:00:00.155162   NodeId - NodeId in MSR C001_100C                        = 0 (0)
00:00:00.155163   TBM - Trailing Bit Manipulation instructions            = 0 (0)
00:00:00.155163   TOPOEXT - Topology Extensions                           = 0 (0)
00:00:00.155164   PRFEXTCORE - Performance Counter Extensions support     = 0 (0)
00:00:00.155165   PRFEXTNB - NB Performance Counter Extensions support    = 0 (0)
00:00:00.155165   DATABPEXT - Data-access Breakpoint Extension            = 0 (0)
00:00:00.155166   PERFTSC - Performance Time Stamp Counter                = 0 (0)
00:00:00.155167   PCX_L2I - L2I/L3 Performance Counter Extensions         = 0 (0)
00:00:00.155167   MONITORX - MWAITX and MONITORX instructions             = 0 (0)
00:00:00.155168   AddrMaskExt - BP Addressing masking extended to bit 31  = 0 (0)
00:00:00.155168 Full Name:                       "        Intel(R) Core(TM) i7-3770 CPU @ 3.40GHz"
00:00:00.155169 TLB 2/4M Instr/Uni:              res0     0 entries
00:00:00.155169 TLB 2/4M Data:                   res0     0 entries
00:00:00.155170 TLB 4K Instr/Uni:                res0     0 entries
00:00:00.155170 TLB 4K Data:                     res0     0 entries
00:00:00.155171 L1 Instr Cache Line Size:        0 bytes
00:00:00.155171 L1 Instr Cache Lines Per Tag:    0
00:00:00.155171 L1 Instr Cache Associativity:    res0
00:00:00.155171 L1 Instr Cache Size:             0 KB
00:00:00.155172 L1 Data Cache Line Size:         0 bytes
00:00:00.155172 L1 Data Cache Lines Per Tag:     0
00:00:00.155172 L1 Data Cache Associativity:     res0
00:00:00.155173 L1 Data Cache Size:              0 KB
00:00:00.155173 L2 TLB 2/4M Instr/Uni:           off       0 entries
00:00:00.155173 L2 TLB 2/4M Data:                off       0 entries
00:00:00.155174 L2 TLB 4K Instr/Uni:             off       0 entries
00:00:00.155174 L2 TLB 4K Data:                  off       0 entries
00:00:00.155174 L2 Cache Line Size:              64 bytes
00:00:00.155175 L2 Cache Lines Per Tag:          0
00:00:00.155175 L2 Cache Associativity:          8 way
00:00:00.155175 L2 Cache Size:                   256 KB
00:00:00.155176 L3 Cache Line Size:              0 bytes
00:00:00.155176 L3 Cache Lines Per Tag:          0
00:00:00.155176 L3 Cache Associativity:          off
00:00:00.155176 L3 Cache Size:                   0 KB
00:00:00.155177 APM Features EDX
00:00:00.155177   Mnemonic - Description                                  = Guest (Host)
00:00:00.155178   TS - Temperature Sensor                                 = 0 (0)
00:00:00.155179   FID - Frequency ID control                              = 0 (0)
00:00:00.155180   VID - Voltage ID control                                = 0 (0)
00:00:00.155181   TTP - Thermal Trip                                      = 0 (0)
00:00:00.155182   TM - Hardware Thermal Control (HTC)                     = 0 (0)
00:00:00.155182   100MHzSteps - 100 MHz Multiplier control                = 0 (0)
00:00:00.155183   HwPstate - Hardware P-state control                     = 0 (0)
00:00:00.155184   TscInvariant - Invariant Time Stamp Counter             = 1 (1)
00:00:00.155184   CPB - Core Performance Boost                            = 0 (0)
00:00:00.155185   EffFreqRO - Read-only Effective Frequency Interface     = 0 (0)
00:00:00.155186   ProcFdbkIf - Processor Feedback Interface               = 0 (0)
00:00:00.155186   ProcPwrRep - Core power reporting interface support     = 0 (0)
00:00:00.155187   ConnectedStandby - Connected Standby                    = 0 (0)
00:00:00.155188   RAPL - Running average power limit                      = 0 (0)
00:00:00.155188 Physical Address Width:          36 bits
00:00:00.155189 Virtual Address Width:           48 bits
00:00:00.155189 Max page count for INVLPGB:      0x3024
00:00:00.155189 Max ECX for RDPRU:               0x0
00:00:00.155190 ********************* End of CPUID dump *********************
00:00:00.155191 *********************** VT-x features ***********************
00:00:00.155191 Nested hardware virtualization - VMX features
00:00:00.155192   Mnemonic - Description                                  = guest (host)
00:00:00.155192   VMX - Virtual-Machine Extensions                        = 0 (1)
00:00:00.155192   InsOutInfo - INS/OUTS instruction info.                 = 0 (1)
00:00:00.155193   ExtIntExit - External interrupt exiting                 = 0 (1)
00:00:00.155193   NmiExit - NMI exiting                                   = 0 (1)
00:00:00.155194   VirtNmi - Virtual NMIs                                  = 0 (1)
00:00:00.155194   PreemptTimer - VMX preemption timer                     = 0 (1)
00:00:00.155194   PostedInt - Posted interrupts                           = 0 (0)
00:00:00.155195   IntWindowExit - Interrupt-window exiting                = 0 (1)
00:00:00.155195   TscOffsetting - TSC offsetting                          = 0 (1)
00:00:00.155195   HltExit - HLT exiting                                   = 0 (1)
00:00:00.155196   InvlpgExit - INVLPG exiting                             = 0 (1)
00:00:00.155196   MwaitExit - MWAIT exiting                               = 0 (1)
00:00:00.155197   RdpmcExit - RDPMC exiting                               = 0 (1)
00:00:00.155197   RdtscExit - RDTSC exiting                               = 0 (1)
00:00:00.155197   Cr3LoadExit - CR3-load exiting                          = 0 (1)
00:00:00.155198   Cr3StoreExit - CR3-store exiting                        = 0 (1)
00:00:00.155198   TertiaryExecCtls - Activate tertiary controls           = 0 (0)
00:00:00.155199   Cr8LoadExit  - CR8-load exiting                         = 0 (1)
00:00:00.155199   Cr8StoreExit - CR8-store exiting                        = 0 (1)
00:00:00.155199   UseTprShadow - Use TPR shadow                           = 0 (1)
00:00:00.155200   NmiWindowExit - NMI-window exiting                      = 0 (1)
00:00:00.155200   MovDRxExit - Mov-DR exiting                             = 0 (1)
00:00:00.155200   UncondIoExit - Unconditional I/O exiting                = 0 (1)
00:00:00.155201   UseIoBitmaps - Use I/O bitmaps                          = 0 (1)
00:00:00.155201   MonitorTrapFlag - Monitor Trap Flag                     = 0 (1)
00:00:00.155202   UseMsrBitmaps - MSR bitmaps                             = 0 (1)
00:00:00.155202   MonitorExit - MONITOR exiting                           = 0 (1)
00:00:00.155202   PauseExit - PAUSE exiting                               = 0 (1)
00:00:00.155203   SecondaryExecCtl - Activate secondary controls          = 0 (1)
00:00:00.155203   VirtApic - Virtualize-APIC accesses                     = 0 (1)
00:00:00.155204   Ept - Extended Page Tables                              = 0 (1)
00:00:00.155204   DescTableExit - Descriptor-table exiting                = 0 (1)
00:00:00.155204   Rdtscp - Enable RDTSCP                                  = 0 (1)
00:00:00.155205   VirtX2ApicMode - Virtualize-x2APIC mode                 = 0 (1)
00:00:00.155205   Vpid - Enable VPID                                      = 0 (1)
00:00:00.155205   WbinvdExit - WBINVD exiting                             = 0 (1)
00:00:00.155206   UnrestrictedGuest - Unrestricted guest                  = 0 (1)
00:00:00.155206   ApicRegVirt - APIC-register virtualization              = 0 (0)
00:00:00.155207   VirtIntDelivery - Virtual-interrupt delivery            = 0 (0)
00:00:00.155207   PauseLoopExit - PAUSE-loop exiting                      = 0 (0)
00:00:00.155207   RdrandExit - RDRAND exiting                             = 0 (1)
00:00:00.155208   Invpcid - Enable INVPCID                                = 0 (0)
00:00:00.155208   VmFuncs - Enable VM Functions                           = 0 (0)
00:00:00.155209   VmcsShadowing - VMCS shadowing                          = 0 (0)
00:00:00.155209   RdseedExiting - RDSEED exiting                          = 0 (0)
00:00:00.155209   PML - Page-Modification Log                             = 0 (0)
00:00:00.155210   EptVe - EPT violations can cause #VE                    = 0 (0)
00:00:00.155210   ConcealVmxFromPt - Conceal VMX from Processor Trace     = 0 (0)
00:00:00.155210   XsavesXRstors - Enable XSAVES/XRSTORS                   = 0 (0)
00:00:00.155211   PasidTranslate - PASID translation                      = 0 (0)
00:00:00.155211   ModeBasedExecuteEpt - Mode-based execute permissions    = 0 (0)
00:00:00.155212   SppEpt - Sub-page page write permissions for EPT        = 0 (0)
00:00:00.155212   PtEpt - Processor Trace address' translatable by EPT    = 0 (0)
00:00:00.155212   UseTscScaling - Use TSC scaling                         = 0 (0)
00:00:00.155213   UserWaitPause - Enable TPAUSE, UMONITOR and UMWAIT      = 0 (0)
00:00:00.155213   Pconfig - Enable PCONFIG                                = 0 (0)
00:00:00.155213   EnclvExit - ENCLV exiting                               = 0 (0)
00:00:00.155214   BusLockDetect - VMM Bus-Lock detection                  = 0 (0)
00:00:00.155214   InstrTimeout - Instruction timeout                      = 0 (0)
00:00:00.155215   LoadIwKeyExit - LOADIWKEY exiting                       = 0 (0)
00:00:00.155215   HLAT - Hypervisor-managed linear-address translation    = 0 (0)
00:00:00.155215   EptPagingWrite - EPT paging-write                       = 0 (0)
00:00:00.155216   GstPagingVerify - Guest-paging verification             = 0 (0)
00:00:00.155216   IpiVirt - IPI virtualization                            = 0 (0)
00:00:00.155217   VirtSpecCtrl - Virtualize IA32_SPEC_CTRL                = 0 (0)
00:00:00.155217   EntryLoadDebugCtls - Load debug controls on VM-entry    = 0 (1)
00:00:00.155217   Ia32eModeGuest - IA-32e mode guest                      = 0 (1)
00:00:00.155218   EntryLoadEferMsr - Load IA32_EFER MSR on VM-entry       = 0 (1)
00:00:00.155218   EntryLoadPatMsr - Load IA32_PAT MSR on VM-entry         = 0 (1)
00:00:00.155218   ExitSaveDebugCtls - Save debug controls on VM-exit      = 0 (1)
00:00:00.155219   HostAddrSpaceSize - Host address-space size             = 0 (1)
00:00:00.155219   ExitAckExtInt - Acknowledge interrupt on VM-exit        = 0 (1)
00:00:00.155220   ExitSavePatMsr - Save IA32_PAT MSR on VM-exit           = 0 (1)
00:00:00.155220   ExitLoadPatMsr - Load IA32_PAT MSR on VM-exit           = 0 (1)
00:00:00.155220   ExitSaveEferMsr - Save IA32_EFER MSR on VM-exit         = 0 (1)
00:00:00.155221   ExitLoadEferMsr - Load IA32_EFER MSR on VM-exit         = 0 (1)
00:00:00.155221   SavePreemptTimer - Save VMX-preemption timer            = 0 (1)
00:00:00.155222   SecondaryExitCtls - Secondary VM-exit controls          = 0 (0)
00:00:00.155222   ExitSaveEferLma - Save IA32_EFER.LMA on VM-exit         = 0 (1)
00:00:00.155222   IntelPt - Intel Processor Trace in VMX operation        = 0 (0)
00:00:00.155223   VmwriteAll - VMWRITE to any supported VMCS field        = 0 (0)
00:00:00.155223   EntryInjectSoftInt - Inject softint. with 0-len instr.  = 0 (0)
00:00:00.155224
00:00:00.155224 ******************* End of VT-x features ********************
00:00:00.155310 VMEmt: Halt method global1 (5)
00:00:00.155382 VMEmt: HaltedGlobal1 config: cNsSpinBlockThresholdCfg=2000
00:00:00.155464 Changing the VM state from 'CREATING' to 'CREATED'
00:00:00.158088 NAT: DNS settings changed, triggering update
00:00:00.158099 Nameserver is either on 127/8 network or failed to obtain from host. Falling back to libslirp DNS proxy.
00:00:00.158102 fallback virtual nameserver: 50462730Changing the VM state from 'CREATED' to 'POWERING_ON'
00:00:00.158645 Changing the VM state from 'POWERING_ON' to 'RUNNING'
00:00:00.158651 Console: Machine state changed to 'Running'
00:00:00.159963 VBoxHeadless: starting event loop
00:00:00.159991 VMMDev: Guest Log: BIOS: VirtualBox 7.2.12
00:00:00.160145 PCI: Setting up resources and interrupts
00:00:00.161763 PIT: mode=2 count=0x10000 (65536) - 18.20 Hz (ch=0)
00:00:00.186514 Display::i_handleDisplayResize: uScreenId=0 pvVRAM=0000000000000000 w=720 h=400 bpp=0 cbLine=0x0 flags=0x0 origin=0,0
00:00:00.186757 VMMDev: Guest Log: CPUID EDX: 0x178bfbbf
00:00:00.186815 PIIX3 ATA: Ctl#0: RESET, DevSel=0 AIOIf=0 CmdIf0=0x00 (-1 usec ago) CmdIf1=0x00 (-1 usec ago)
00:00:00.186834 PIIX3 ATA: Ctl#0: finished processing RESET
00:00:00.187146 VMMDev: Guest Log: BIOS: ata0-0: PCHS=4161/16/63 LCHS=520/128/63
00:00:00.187720 PIIX3 ATA: Ctl#1: RESET, DevSel=0 AIOIf=0 CmdIf0=0x00 (-1 usec ago) CmdIf1=0x00 (-1 usec ago)
00:00:00.187768 PIIX3 ATA: Ctl#1: finished processing RESET
00:00:00.190589 PIT: mode=2 count=0x48d3 (18643) - 64.00 Hz (ch=0)
00:00:00.216731 Display::i_handleDisplayResize: uScreenId=0 pvVRAM=00007fd510000000 w=640 h=480 bpp=32 cbLine=0xA00 flags=0x0 origin=0,0
00:00:02.817195 PIT: mode=2 count=0x10000 (65536) - 18.20 Hz (ch=0)
00:00:02.817569 VMMDev: Guest Log: BIOS: Boot : bseqnr=1, bootseq=0231
00:00:02.817843 VMMDev: Guest Log: BIOS: Boot from Floppy 0 failed
00:00:02.818078 VMMDev: Guest Log: BIOS: Boot : bseqnr=2, bootseq=0023
00:00:02.818632 VMMDev: Guest Log: BIOS: Booting from CD-ROM...
00:00:02.831223 Display::i_handleDisplayResize: uScreenId=0 pvVRAM=0000000000000000 w=720 h=400 bpp=0 cbLine=0x0 flags=0x0 origin=0,0
00:00:02.945981 Changing the VM state from 'RUNNING' to 'GURU_MEDITATION'
00:00:02.946005 Console: Machine state changed to 'Stuck'
00:00:02.946411 !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
00:00:02.946419 !!
00:00:02.946420 !!         VCPU0: Guru Meditation 1155 (VINF_EM_TRIPLE_FAULT)
00:00:02.946424 !!
00:00:02.946427 !! Skipping ring-0 registers and stack, rcErr=VINF_EM_TRIPLE_FAULT
00:00:02.946429 !!
00:00:02.946429 !! {mappings, <NULL>}
00:00:02.946430 !!
00:00:02.946439 !!
00:00:02.946439 !! {hma, <NULL>}
00:00:02.946440 !!
00:00:02.946441 !!
00:00:02.946441 !! {cpumguest, verbose}
00:00:02.946442 !!
00:00:02.946446 Guest CPUM (VCPU 0) state:
00:00:02.946449 eax=00000030 ebx=00245a00 ecx=00000000 edx=000003f8 esi=00000000 edi=00108000
00:00:02.946451 eip=001010f4 esp=00007c00 ebp=36d76289 iopl=0  iopl=0 nv up di pl nz na po nc
00:00:02.946453 cs={0008 base=0000000000000000 limit=ffffffff flags=0000c09b} dr0=00000000 dr1=00000000
00:00:02.946456 ds={0010 base=0000000000000000 limit=ffffffff flags=0000c093} dr2=00000000 dr3=00000000
00:00:02.946458 es={0010 base=0000000000000000 limit=ffffffff flags=0000c093} dr4=00000000 dr5=00000000
00:00:02.946460 fs={0010 base=0000000000000000 limit=ffffffff flags=0000c093} dr6=ffff0ff0 dr7=00000400
00:00:02.946462 gs={0010 base=0000000000000000 limit=ffffffff flags=0000c093} cr0=00000011 cr2=00000000
00:00:02.946464 ss={0010 base=0000000000000000 limit=ffffffff flags=0000c093} cr3=00000000 cr4=00000000
00:00:02.946465 gdtr=0000000000101130:0017  idtr=0000000000000000:0000  eflags=00000006
00:00:02.946467 ldtr={0000 base=00000000 limit=0000ffff flags=00000082}
00:00:02.946468 tr  ={0000 base=00000000 limit=0000ffff flags=0000008b}
00:00:02.946469 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.946479 xcr=0000000000000001 xcr1=0000000000000000 xss=0000000000000000 (fXStateMask=0000000000000000)
00:00:02.946481 FCW=037f FSW=0000 FTW=0000 FOP=0000 MXCSR=00001f80 MXCSR_MASK=0000ffff
00:00:02.946482 FPUIP=00000000 CS=0000 Rsrvd1=0000  FPUDP=00000000 DS=0000 Rsvrd2=0000
00:00:02.946484 ST(0)=FPR0={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946489 ST(1)=FPR1={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946492 ST(2)=FPR2={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946495 ST(3)=FPR3={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946498 ST(4)=FPR4={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946502 ST(5)=FPR5={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946504 ST(6)=FPR6={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946508 ST(7)=FPR7={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946511 XMM0 =00000000'00000000'00000000'00000000  XMM1 =00000000'00000000'00000000'00000000
00:00:02.946516 XMM2 =00000000'00000000'00000000'00000000  XMM3 =00000000'00000000'00000000'00000000
00:00:02.946518 XMM4 =00000000'00000000'00000000'00000000  XMM5 =00000000'00000000'00000000'00000000
00:00:02.946523 XMM6 =00000000'00000000'00000000'00000000  XMM7 =00000000'00000000'00000000'00000000
00:00:02.946526 XMM8 =00000000'00000000'00000000'00000000  XMM9 =00000000'00000000'00000000'00000000
00:00:02.946530 XMM10=00000000'00000000'00000000'00000000  XMM11=00000000'00000000'00000000'00000000
00:00:02.946535 XMM12=00000000'00000000'00000000'00000000  XMM13=00000000'00000000'00000000'00000000
00:00:02.946538 XMM14=00000000'00000000'00000000'00000000  XMM15=00000000'00000000'00000000'00000000
00:00:02.946543 EFER         =0000000000000000
00:00:02.946543 PAT          =0007040600070406
00:00:02.946544 STAR         =0000000000000000
00:00:02.946545 CSTAR        =0000000000000000
00:00:02.946545 LSTAR        =0000000000000000
00:00:02.946546 SFMASK       =0000000000000000
00:00:02.946546 KERNELGSBASE =0000000000000000
00:00:02.946549 MTRR_CAP          =0000000000000510
00:00:02.946549 MTRR_DEF_TYPE     =0000000000000c00
00:00:02.946550 MTRR_FIX64K_00000 =0606060606060606
00:00:02.946551 MTRR_FIX16K_80000 =0606060606060606
00:00:02.946552 MTRR_FIX16K_A0000 =0000000000000000
00:00:02.946552 MTRR_FIX4K_C0000  =0505050505050505
00:00:02.946553 MTRR_FIX4K_C8000  =0505050505050505
00:00:02.946554 MTRR_FIX4K_D0000  =0505050505050505
00:00:02.946554 MTRR_FIX4K_D8000  =0505050505050505
00:00:02.946555 MTRR_FIX4K_E0000  =0505050505050505
00:00:02.946556 MTRR_FIX4K_E8000  =0505050505050505
00:00:02.946557 MTRR_FIX4K_F0000  =0505050505050505
00:00:02.946557 MTRR_FIX4K_F8000  =0505050505050505
00:00:02.946565 MTRR_PHYSBASE[ 0] =0000000000000006 First=0000000000000000      0 MB [WB]
00:00:02.946567 MTRR_PHYSMASK[ 0] =0000000f00000800 Last =00000000ffffffff   4095 MB [4096 MB]
00:00:02.946571 MTRR_PHYSBASE[ 1] =00000000e0000000 First=00000000e0000000   3584 MB [UC]
00:00:02.946573 MTRR_PHYSMASK[ 1] =0000000fe0000800 Last =00000000ffffffff   4095 MB [512 MB]
00:00:02.946577 MTRR_PHYSBASE[ 2] =0000000100000006 First=0000000100000000   4096 MB [WB]
00:00:02.946579 MTRR_PHYSMASK[ 2] =0000000fe0000800 Last =000000011fffffff   4607 MB [512 MB]
00:00:02.946583 MTRR_PHYSBASE[ 3] =0000000000000000
00:00:02.946584 MTRR_PHYSMASK[ 3] =0000000000000000
00:00:02.946586 MTRR_PHYSBASE[ 4] =0000000000000000
00:00:02.946586 MTRR_PHYSMASK[ 4] =0000000000000000
00:00:02.946588 MTRR_PHYSBASE[ 5] =0000000000000000
00:00:02.946589 MTRR_PHYSMASK[ 5] =0000000000000000
00:00:02.946591 MTRR_PHYSBASE[ 6] =0000000000000000
00:00:02.946591 MTRR_PHYSMASK[ 6] =0000000000000000
00:00:02.946592 MTRR_PHYSBASE[ 7] =0000000000000000
00:00:02.946592 MTRR_PHYSMASK[ 7] =0000000000000000
00:00:02.946593 MTRR_PHYSBASE[ 8] =0000000000000000
00:00:02.946594 MTRR_PHYSMASK[ 8] =0000000000000000
00:00:02.946594 MTRR_PHYSBASE[ 9] =0000000000000000
00:00:02.946595 MTRR_PHYSMASK[ 9] =0000000000000000
00:00:02.946596 MTRR_PHYSBASE[10] =0000000000000000
00:00:02.946596 MTRR_PHYSMASK[10] =0000000000000000
00:00:02.946597 MTRR_PHYSBASE[11] =0000000000000000
00:00:02.946597 MTRR_PHYSMASK[11] =0000000000000000
00:00:02.946598 MTRR_PHYSBASE[12] =0000000000000000
00:00:02.946598 MTRR_PHYSMASK[12] =0000000000000000
00:00:02.946599 MTRR_PHYSBASE[13] =0000000000000000
00:00:02.946599 MTRR_PHYSMASK[13] =0000000000000000
00:00:02.946600 MTRR_PHYSBASE[14] =0000000000000000
00:00:02.946601 MTRR_PHYSMASK[14] =0000000000000000
00:00:02.946601 MTRR_PHYSBASE[15] =0000000000000000
00:00:02.946602 MTRR_PHYSMASK[15] =0000000000000000
00:00:02.946627 Guest CPUM (VCPU 1) state:
00:00:02.946629 eax=00000000 ebx=00000000 ecx=00000000 edx=00000600 esi=00000000 edi=00000000
00:00:02.946630 eip=0000fff0 esp=00000000 ebp=00000000 iopl=0  iopl=0 nv up di pl nz na pe nc
00:00:02.946631 cs={f000 base=00000000ffff0000 limit=0000ffff flags=0000009b} dr0=00000000 dr1=00000000
00:00:02.946634 ds={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr2=00000000 dr3=00000000
00:00:02.946636 es={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr4=00000000 dr5=00000000
00:00:02.946638 fs={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr6=ffff0ff0 dr7=00000400
00:00:02.946639 gs={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr0=60000010 cr2=00000000
00:00:02.946641 ss={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr3=00000000 cr4=00000000
00:00:02.946642 gdtr=0000000000000000:ffff  idtr=0000000000000000:ffff  eflags=00000002
00:00:02.946643 ldtr={0000 base=00000000 limit=0000ffff flags=00000082}
00:00:02.946644 tr  ={0000 base=00000000 limit=0000ffff flags=0000008b}
00:00:02.946645 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.946657 xcr=0000000000000001 xcr1=0000000000000000 xss=0000000000000000 (fXStateMask=0000000000000000)
00:00:02.946659 FCW=037f FSW=0000 FTW=0000 FOP=0000 MXCSR=00001f80 MXCSR_MASK=0000ffff
00:00:02.946674 FPUIP=00000000 CS=0000 Rsrvd1=0000  FPUDP=00000000 DS=0000 Rsvrd2=0000
00:00:02.946677 ST(0)=FPR0={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946683 ST(1)=FPR1={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946686 ST(2)=FPR2={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946690 ST(3)=FPR3={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946693 ST(4)=FPR4={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946697 ST(5)=FPR5={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946700 ST(6)=FPR6={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946703 ST(7)=FPR7={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946706 XMM0 =00000000'00000000'00000000'00000000  XMM1 =00000000'00000000'00000000'00000000
00:00:02.946711 XMM2 =00000000'00000000'00000000'00000000  XMM3 =00000000'00000000'00000000'00000000
00:00:02.946716 XMM4 =00000000'00000000'00000000'00000000  XMM5 =00000000'00000000'00000000'00000000
00:00:02.946720 XMM6 =00000000'00000000'00000000'00000000  XMM7 =00000000'00000000'00000000'00000000
00:00:02.946724 XMM8 =00000000'00000000'00000000'00000000  XMM9 =00000000'00000000'00000000'00000000
00:00:02.946728 XMM10=00000000'00000000'00000000'00000000  XMM11=00000000'00000000'00000000'00000000
00:00:02.946733 XMM12=00000000'00000000'00000000'00000000  XMM13=00000000'00000000'00000000'00000000
00:00:02.946737 XMM14=00000000'00000000'00000000'00000000  XMM15=00000000'00000000'00000000'00000000
00:00:02.946741 EFER         =0000000000000000
00:00:02.946742 PAT          =0007040600070406
00:00:02.946743 STAR         =0000000000000000
00:00:02.946743 CSTAR        =0000000000000000
00:00:02.946744 LSTAR        =0000000000000000
00:00:02.946745 SFMASK       =0000000000000000
00:00:02.946745 KERNELGSBASE =0000000000000000
00:00:02.946748 MTRR_CAP          =0000000000000510
00:00:02.946749 MTRR_DEF_TYPE     =0000000000000c00
00:00:02.946749 MTRR_FIX64K_00000 =0606060606060606
00:00:02.946750 MTRR_FIX16K_80000 =0606060606060606
00:00:02.946751 MTRR_FIX16K_A0000 =0000000000000000
00:00:02.946752 MTRR_FIX4K_C0000  =0505050505050505
00:00:02.946753 MTRR_FIX4K_C8000  =0505050505050505
00:00:02.946753 MTRR_FIX4K_D0000  =0505050505050505
00:00:02.946754 MTRR_FIX4K_D8000  =0505050505050505
00:00:02.946755 MTRR_FIX4K_E0000  =0505050505050505
00:00:02.946755 MTRR_FIX4K_E8000  =0505050505050505
00:00:02.946756 MTRR_FIX4K_F0000  =0505050505050505
00:00:02.946756 MTRR_FIX4K_F8000  =0505050505050505
00:00:02.946763 MTRR_PHYSBASE[ 0] =0000000000000006 First=0000000000000000      0 MB [WB]
00:00:02.946764 MTRR_PHYSMASK[ 0] =0000000f00000800 Last =00000000ffffffff   4095 MB [4096 MB]
00:00:02.946770 MTRR_PHYSBASE[ 1] =00000000e0000000 First=00000000e0000000   3584 MB [UC]
00:00:02.946771 MTRR_PHYSMASK[ 1] =0000000fe0000800 Last =00000000ffffffff   4095 MB [512 MB]
00:00:02.946776 MTRR_PHYSBASE[ 2] =0000000100000006 First=0000000100000000   4096 MB [WB]
00:00:02.946778 MTRR_PHYSMASK[ 2] =0000000fe0000800 Last =000000011fffffff   4607 MB [512 MB]
00:00:02.946783 MTRR_PHYSBASE[ 3] =0000000000000000
00:00:02.946783 MTRR_PHYSMASK[ 3] =0000000000000000
00:00:02.946784 MTRR_PHYSBASE[ 4] =0000000000000000
00:00:02.946785 MTRR_PHYSMASK[ 4] =0000000000000000
00:00:02.946786 MTRR_PHYSBASE[ 5] =0000000000000000
00:00:02.946786 MTRR_PHYSMASK[ 5] =0000000000000000
00:00:02.946787 MTRR_PHYSBASE[ 6] =0000000000000000
00:00:02.946787 MTRR_PHYSMASK[ 6] =0000000000000000
00:00:02.946788 MTRR_PHYSBASE[ 7] =0000000000000000
00:00:02.946788 MTRR_PHYSMASK[ 7] =0000000000000000
00:00:02.946789 MTRR_PHYSBASE[ 8] =0000000000000000
00:00:02.946789 MTRR_PHYSMASK[ 8] =0000000000000000
00:00:02.946790 MTRR_PHYSBASE[ 9] =0000000000000000
00:00:02.946791 MTRR_PHYSMASK[ 9] =0000000000000000
00:00:02.946791 MTRR_PHYSBASE[10] =0000000000000000
00:00:02.946792 MTRR_PHYSMASK[10] =0000000000000000
00:00:02.946793 MTRR_PHYSBASE[11] =0000000000000000
00:00:02.946793 MTRR_PHYSMASK[11] =0000000000000000
00:00:02.946794 MTRR_PHYSBASE[12] =0000000000000000
00:00:02.946794 MTRR_PHYSMASK[12] =0000000000000000
00:00:02.946795 MTRR_PHYSBASE[13] =0000000000000000
00:00:02.946795 MTRR_PHYSMASK[13] =0000000000000000
00:00:02.946796 MTRR_PHYSBASE[14] =0000000000000000
00:00:02.946796 MTRR_PHYSMASK[14] =0000000000000000
00:00:02.946797 MTRR_PHYSBASE[15] =0000000000000000
00:00:02.946798 MTRR_PHYSMASK[15] =0000000000000000
00:00:02.946824 Guest CPUM (VCPU 2) state:
00:00:02.946827 eax=00000000 ebx=00000000 ecx=00000000 edx=00000600 esi=00000000 edi=00000000
00:00:02.946828 eip=0000fff0 esp=00000000 ebp=00000000 iopl=0  iopl=0 nv up di pl nz na pe nc
00:00:02.946830 cs={f000 base=00000000ffff0000 limit=0000ffff flags=0000009b} dr0=00000000 dr1=00000000
00:00:02.946832 ds={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr2=00000000 dr3=00000000
00:00:02.946834 es={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr4=00000000 dr5=00000000
00:00:02.946835 fs={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr6=ffff0ff0 dr7=00000400
00:00:02.946837 gs={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr0=60000010 cr2=00000000
00:00:02.946839 ss={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr3=00000000 cr4=00000000
00:00:02.946840 gdtr=0000000000000000:ffff  idtr=0000000000000000:ffff  eflags=00000002
00:00:02.946842 ldtr={0000 base=00000000 limit=0000ffff flags=00000082}
00:00:02.946843 tr  ={0000 base=00000000 limit=0000ffff flags=0000008b}
00:00:02.946844 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.946855 xcr=0000000000000001 xcr1=0000000000000000 xss=0000000000000000 (fXStateMask=0000000000000000)
00:00:02.946858 FCW=037f FSW=0000 FTW=0000 FOP=0000 MXCSR=00001f80 MXCSR_MASK=0000ffff
00:00:02.946878 FPUIP=00000000 CS=0000 Rsrvd1=0000  FPUDP=00000000 DS=0000 Rsvrd2=0000
00:00:02.946881 ST(0)=FPR0={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946885 ST(1)=FPR1={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946889 ST(2)=FPR2={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946892 ST(3)=FPR3={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946895 ST(4)=FPR4={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946898 ST(5)=FPR5={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946902 ST(6)=FPR6={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946905 ST(7)=FPR7={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.946908 XMM0 =00000000'00000000'00000000'00000000  XMM1 =00000000'00000000'00000000'00000000
00:00:02.946913 XMM2 =00000000'00000000'00000000'00000000  XMM3 =00000000'00000000'00000000'00000000
00:00:02.946917 XMM4 =00000000'00000000'00000000'00000000  XMM5 =00000000'00000000'00000000'00000000
00:00:02.946922 XMM6 =00000000'00000000'00000000'00000000  XMM7 =00000000'00000000'00000000'00000000
00:00:02.946927 XMM8 =00000000'00000000'00000000'00000000  XMM9 =00000000'00000000'00000000'00000000
00:00:02.946931 XMM10=00000000'00000000'00000000'00000000  XMM11=00000000'00000000'00000000'00000000
00:00:02.946936 XMM12=00000000'00000000'00000000'00000000  XMM13=00000000'00000000'00000000'00000000
00:00:02.946940 XMM14=00000000'00000000'00000000'00000000  XMM15=00000000'00000000'00000000'00000000
00:00:02.946945 EFER         =0000000000000000
00:00:02.946945 PAT          =0007040600070406
00:00:02.946946 STAR         =0000000000000000
00:00:02.946947 CSTAR        =0000000000000000
00:00:02.946948 LSTAR        =0000000000000000
00:00:02.946948 SFMASK       =0000000000000000
00:00:02.946949 KERNELGSBASE =0000000000000000
00:00:02.946952 MTRR_CAP          =0000000000000510
00:00:02.946952 MTRR_DEF_TYPE     =0000000000000c00
00:00:02.946953 MTRR_FIX64K_00000 =0606060606060606
00:00:02.946954 MTRR_FIX16K_80000 =0606060606060606
00:00:02.946955 MTRR_FIX16K_A0000 =0000000000000000
00:00:02.946955 MTRR_FIX4K_C0000  =0505050505050505
00:00:02.946956 MTRR_FIX4K_C8000  =0505050505050505
00:00:02.946957 MTRR_FIX4K_D0000  =0505050505050505
00:00:02.946958 MTRR_FIX4K_D8000  =0505050505050505
00:00:02.946959 MTRR_FIX4K_E0000  =0505050505050505
00:00:02.946959 MTRR_FIX4K_E8000  =0505050505050505
00:00:02.946960 MTRR_FIX4K_F0000  =0505050505050505
00:00:02.946961 MTRR_FIX4K_F8000  =0505050505050505
00:00:02.946968 MTRR_PHYSBASE[ 0] =0000000000000006 First=0000000000000000      0 MB [WB]
00:00:02.946970 MTRR_PHYSMASK[ 0] =0000000f00000800 Last =00000000ffffffff   4095 MB [4096 MB]
00:00:02.946975 MTRR_PHYSBASE[ 1] =00000000e0000000 First=00000000e0000000   3584 MB [UC]
00:00:02.946976 MTRR_PHYSMASK[ 1] =0000000fe0000800 Last =00000000ffffffff   4095 MB [512 MB]
00:00:02.946980 MTRR_PHYSBASE[ 2] =0000000100000006 First=0000000100000000   4096 MB [WB]
00:00:02.946982 MTRR_PHYSMASK[ 2] =0000000fe0000800 Last =000000011fffffff   4607 MB [512 MB]
00:00:02.946986 MTRR_PHYSBASE[ 3] =0000000000000000
00:00:02.946987 MTRR_PHYSMASK[ 3] =0000000000000000
00:00:02.946989 MTRR_PHYSBASE[ 4] =0000000000000000
00:00:02.946989 MTRR_PHYSMASK[ 4] =0000000000000000
00:00:02.946991 MTRR_PHYSBASE[ 5] =0000000000000000
00:00:02.946991 MTRR_PHYSMASK[ 5] =0000000000000000
00:00:02.946993 MTRR_PHYSBASE[ 6] =0000000000000000
00:00:02.946993 MTRR_PHYSMASK[ 6] =0000000000000000
00:00:02.946995 MTRR_PHYSBASE[ 7] =0000000000000000
00:00:02.946996 MTRR_PHYSMASK[ 7] =0000000000000000
00:00:02.946997 MTRR_PHYSBASE[ 8] =0000000000000000
00:00:02.946998 MTRR_PHYSMASK[ 8] =0000000000000000
00:00:02.946999 MTRR_PHYSBASE[ 9] =0000000000000000
00:00:02.947000 MTRR_PHYSMASK[ 9] =0000000000000000
00:00:02.947001 MTRR_PHYSBASE[10] =0000000000000000
00:00:02.947002 MTRR_PHYSMASK[10] =0000000000000000
00:00:02.947003 MTRR_PHYSBASE[11] =0000000000000000
00:00:02.947004 MTRR_PHYSMASK[11] =0000000000000000
00:00:02.947005 MTRR_PHYSBASE[12] =0000000000000000
00:00:02.947006 MTRR_PHYSMASK[12] =0000000000000000
00:00:02.947008 MTRR_PHYSBASE[13] =0000000000000000
00:00:02.947008 MTRR_PHYSMASK[13] =0000000000000000
00:00:02.947010 MTRR_PHYSBASE[14] =0000000000000000
00:00:02.947010 MTRR_PHYSMASK[14] =0000000000000000
00:00:02.947012 MTRR_PHYSBASE[15] =0000000000000000
00:00:02.947012 MTRR_PHYSMASK[15] =0000000000000000
00:00:02.947067 Guest CPUM (VCPU 3) state:
00:00:02.947070 eax=00000000 ebx=00000000 ecx=00000000 edx=00000600 esi=00000000 edi=00000000
00:00:02.947071 eip=0000fff0 esp=00000000 ebp=00000000 iopl=0  iopl=0 nv up di pl nz na pe nc
00:00:02.947072 cs={f000 base=00000000ffff0000 limit=0000ffff flags=0000009b} dr0=00000000 dr1=00000000
00:00:02.947074 ds={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr2=00000000 dr3=00000000
00:00:02.947075 es={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr4=00000000 dr5=00000000
00:00:02.947076 fs={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr6=ffff0ff0 dr7=00000400
00:00:02.947077 gs={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr0=60000010 cr2=00000000
00:00:02.947078 ss={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr3=00000000 cr4=00000000
00:00:02.947079 gdtr=0000000000000000:ffff  idtr=0000000000000000:ffff  eflags=00000002
00:00:02.947080 ldtr={0000 base=00000000 limit=0000ffff flags=00000082}
00:00:02.947081 tr  ={0000 base=00000000 limit=0000ffff flags=0000008b}
00:00:02.947082 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.947090 xcr=0000000000000001 xcr1=0000000000000000 xss=0000000000000000 (fXStateMask=0000000000000000)
00:00:02.947092 FCW=037f FSW=0000 FTW=0000 FOP=0000 MXCSR=00001f80 MXCSR_MASK=0000ffff
00:00:02.947093 FPUIP=00000000 CS=0000 Rsrvd1=0000  FPUDP=00000000 DS=0000 Rsvrd2=0000
00:00:02.947095 ST(0)=FPR0={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.947099 ST(1)=FPR1={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.947103 ST(2)=FPR2={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.947106 ST(3)=FPR3={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.947109 ST(4)=FPR4={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.947112 ST(5)=FPR5={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.947115 ST(6)=FPR6={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.947119 ST(7)=FPR7={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.947122 XMM0 =00000000'00000000'00000000'00000000  XMM1 =00000000'00000000'00000000'00000000
00:00:02.947127 XMM2 =00000000'00000000'00000000'00000000  XMM3 =00000000'00000000'00000000'00000000
00:00:02.947131 XMM4 =00000000'00000000'00000000'00000000  XMM5 =00000000'00000000'00000000'00000000
00:00:02.947135 XMM6 =00000000'00000000'00000000'00000000  XMM7 =00000000'00000000'00000000'00000000
00:00:02.947140 XMM8 =00000000'00000000'00000000'00000000  XMM9 =00000000'00000000'00000000'00000000
00:00:02.947144 XMM10=00000000'00000000'00000000'00000000  XMM11=00000000'00000000'00000000'00000000
00:00:02.947149 XMM12=00000000'00000000'00000000'00000000  XMM13=00000000'00000000'00000000'00000000
00:00:02.947154 XMM14=00000000'00000000'00000000'00000000  XMM15=00000000'00000000'00000000'00000000
00:00:02.947158 EFER         =0000000000000000
00:00:02.947159 PAT          =0007040600070406
00:00:02.947160 STAR         =0000000000000000
00:00:02.947160 CSTAR        =0000000000000000
00:00:02.947161 LSTAR        =0000000000000000
00:00:02.947162 SFMASK       =0000000000000000
00:00:02.947162 KERNELGSBASE =0000000000000000
00:00:02.947166 MTRR_CAP          =0000000000000510
00:00:02.947166 MTRR_DEF_TYPE     =0000000000000c00
00:00:02.947167 MTRR_FIX64K_00000 =0606060606060606
00:00:02.947168 MTRR_FIX16K_80000 =0606060606060606
00:00:02.947169 MTRR_FIX16K_A0000 =0000000000000000
00:00:02.947169 MTRR_FIX4K_C0000  =0505050505050505
00:00:02.947170 MTRR_FIX4K_C8000  =0505050505050505
00:00:02.947171 MTRR_FIX4K_D0000  =0505050505050505
00:00:02.947172 MTRR_FIX4K_D8000  =0505050505050505
00:00:02.947172 MTRR_FIX4K_E0000  =0505050505050505
00:00:02.947173 MTRR_FIX4K_E8000  =0505050505050505
00:00:02.947174 MTRR_FIX4K_F0000  =0505050505050505
00:00:02.947174 MTRR_FIX4K_F8000  =0505050505050505
00:00:02.947180 MTRR_PHYSBASE[ 0] =0000000000000006 First=0000000000000000      0 MB [WB]
00:00:02.947182 MTRR_PHYSMASK[ 0] =0000000f00000800 Last =00000000ffffffff   4095 MB [4096 MB]
00:00:02.947187 MTRR_PHYSBASE[ 1] =00000000e0000000 First=00000000e0000000   3584 MB [UC]
00:00:02.947188 MTRR_PHYSMASK[ 1] =0000000fe0000800 Last =00000000ffffffff   4095 MB [512 MB]
00:00:02.947192 MTRR_PHYSBASE[ 2] =0000000100000006 First=0000000100000000   4096 MB [WB]
00:00:02.947193 MTRR_PHYSMASK[ 2] =0000000fe0000800 Last =000000011fffffff   4607 MB [512 MB]
00:00:02.947196 MTRR_PHYSBASE[ 3] =0000000000000000
00:00:02.947197 MTRR_PHYSMASK[ 3] =0000000000000000
00:00:02.947197 MTRR_PHYSBASE[ 4] =0000000000000000
00:00:02.947198 MTRR_PHYSMASK[ 4] =0000000000000000
00:00:02.947199 MTRR_PHYSBASE[ 5] =0000000000000000
00:00:02.947199 MTRR_PHYSMASK[ 5] =0000000000000000
00:00:02.947200 MTRR_PHYSBASE[ 6] =0000000000000000
00:00:02.947200 MTRR_PHYSMASK[ 6] =0000000000000000
00:00:02.947201 MTRR_PHYSBASE[ 7] =0000000000000000
00:00:02.947201 MTRR_PHYSMASK[ 7] =0000000000000000
00:00:02.947202 MTRR_PHYSBASE[ 8] =0000000000000000
00:00:02.947203 MTRR_PHYSMASK[ 8] =0000000000000000
00:00:02.947204 MTRR_PHYSBASE[ 9] =0000000000000000
00:00:02.947205 MTRR_PHYSMASK[ 9] =0000000000000000
00:00:02.947206 MTRR_PHYSBASE[10] =0000000000000000
00:00:02.947207 MTRR_PHYSMASK[10] =0000000000000000
00:00:02.947208 MTRR_PHYSBASE[11] =0000000000000000
00:00:02.947209 MTRR_PHYSMASK[11] =0000000000000000
00:00:02.947210 MTRR_PHYSBASE[12] =0000000000000000
00:00:02.947211 MTRR_PHYSMASK[12] =0000000000000000
00:00:02.947212 MTRR_PHYSBASE[13] =0000000000000000
00:00:02.947213 MTRR_PHYSMASK[13] =0000000000000000
00:00:02.947214 MTRR_PHYSBASE[14] =0000000000000000
00:00:02.947215 MTRR_PHYSMASK[14] =0000000000000000
00:00:02.947216 MTRR_PHYSBASE[15] =0000000000000000
00:00:02.947217 MTRR_PHYSMASK[15] =0000000000000000
00:00:02.947255 !!
00:00:02.947256 !! {cpumguesthwvirt, verbose}
00:00:02.947256 !!
00:00:02.947260 VCPU[0] hardware virtualization state:
00:00:02.947261 fSavedInhibit                = 0x0
00:00:02.947262 In nested-guest hwvirt mode  = false
00:00:02.947263 Hwvirt state disabled.
00:00:02.947265 VCPU[1] hardware virtualization state:
00:00:02.947266 fSavedInhibit                = 0x0
00:00:02.947267 In nested-guest hwvirt mode  = false
00:00:02.947268 Hwvirt state disabled.
00:00:02.947283 VCPU[2] hardware virtualization state:
00:00:02.947284 fSavedInhibit                = 0x0
00:00:02.947285 In nested-guest hwvirt mode  = false
00:00:02.947286 Hwvirt state disabled.
00:00:02.947314 VCPU[3] hardware virtualization state:
00:00:02.947315 fSavedInhibit                = 0x0
00:00:02.947317 In nested-guest hwvirt mode  = false
00:00:02.947318 Hwvirt state disabled.
00:00:02.947325 !!
00:00:02.947325 !! {cpumguestinstr, verbose}
00:00:02.947326 !!
00:00:02.947351
00:00:02.947351 CPUM0: 0008:001010f4 0f 22 e0                mov cr4, eax
00:00:02.947351
00:00:02.947363
00:00:02.947364 CPUM1: f000:fff0 ea 5b e0 00 f0          jmp far 0f000h:0e05bh
00:00:02.947364
00:00:02.947383
00:00:02.947384 CPUM2: f000:fff0 ea 5b e0 00 f0          jmp far 0f000h:0e05bh
00:00:02.947384
00:00:02.947397
00:00:02.947398 CPUM3: f000:fff0 ea 5b e0 00 f0          jmp far 0f000h:0e05bh
00:00:02.947398
00:00:02.947406 !!
00:00:02.947406 !! {cpumhyper, verbose}
00:00:02.947407 !!
00:00:02.947410 Hypervisor CPUM state:
00:00:02.947411 .dr0=0000000000000000 .dr1=0000000000000000 .dr2=0000000000000000 .dr3=0000000000000000
00:00:02.947422 .dr4=0000000000000000 .dr5=0000000000000000 .dr6=0000000000000000 .dr7=0000000000000000
00:00:02.947427 Hypervisor CPUM state:
00:00:02.947427 .dr0=0000000000000000 .dr1=0000000000000000 .dr2=0000000000000000 .dr3=0000000000000000
00:00:02.947429 .dr4=0000000000000000 .dr5=0000000000000000 .dr6=0000000000000000 .dr7=0000000000000000
00:00:02.947438 Hypervisor CPUM state:
00:00:02.947438 .dr0=0000000000000000 .dr1=0000000000000000 .dr2=0000000000000000 .dr3=0000000000000000
00:00:02.947439 .dr4=0000000000000000 .dr5=0000000000000000 .dr6=0000000000000000 .dr7=0000000000000000
00:00:02.947465 Hypervisor CPUM state:
00:00:02.947466 .dr0=0000000000000000 .dr1=0000000000000000 .dr2=0000000000000000 .dr3=0000000000000000
00:00:02.947467 .dr4=0000000000000000 .dr5=0000000000000000 .dr6=0000000000000000 .dr7=0000000000000000
00:00:02.947536 !!
00:00:02.947537 !! {cpumhost, verbose}
00:00:02.947538 !!
00:00:02.947541 Host CPUM state:
00:00:02.947542 rax=xxxxxxxxxxxxxxxx rbx=0000000000000000 rcx=xxxxxxxxxxxxxxxx
00:00:02.947543 rdx=xxxxxxxxxxxxxxxx rsi=0000000000000000 rdi=0000000000000000
00:00:02.947544 rip=xxxxxxxxxxxxxxxx rsp=0000000000000000 rbp=0000000000000000
00:00:02.947545  r8=xxxxxxxxxxxxxxxx  r9=xxxxxxxxxxxxxxxx r10=0000000000000000
00:00:02.947545 r11=0000000000000000 r12=0000000000000000 r13=0000000000000000
00:00:02.947546 r14=0000000000000000 r15=0000000000000000
00:00:02.947547 iopl=0   iopl=0 nv up di pl nz na pe nc
00:00:02.947547 cs=0000  ds=0000  es=0000  fs=0000  gs=0000                   eflags=00000000
00:00:02.947548 cr0=0000000000000000 cr2=xxxxxxxxxxxxxxxx cr3=0000000000000000
00:00:02.947548 cr4=0000000000000000 ldtr=0000 tr=0000
00:00:02.947549 dr[0]=0000000000000000 dr[1]=0000000000000000 dr[2]=0000000000000000
00:00:02.947550 dr[3]=0000000000000000 dr[6]=0000000000000000 dr[7]=0000000000000000
00:00:02.947550 gdtr=0000000000000000:0000  idtr=0000000000000000:0000
00:00:02.947551 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.947551 FSbase=0000000000000000 GSbase=0000000000000000 efer=00000000
00:00:02.947560 Host CPUM state:
00:00:02.947561 rax=xxxxxxxxxxxxxxxx rbx=0000000000000000 rcx=xxxxxxxxxxxxxxxx
00:00:02.947562 rdx=xxxxxxxxxxxxxxxx rsi=0000000000000000 rdi=0000000000000000
00:00:02.947563 rip=xxxxxxxxxxxxxxxx rsp=0000000000000000 rbp=0000000000000000
00:00:02.947563  r8=xxxxxxxxxxxxxxxx  r9=xxxxxxxxxxxxxxxx r10=0000000000000000
00:00:02.947564 r11=0000000000000000 r12=0000000000000000 r13=0000000000000000
00:00:02.947565 r14=0000000000000000 r15=0000000000000000
00:00:02.947565 iopl=0   iopl=0 nv up di pl nz na pe nc
00:00:02.947566 cs=0000  ds=0000  es=0000  fs=0000  gs=0000                   eflags=00000000
00:00:02.947567 cr0=0000000000000000 cr2=xxxxxxxxxxxxxxxx cr3=0000000000000000
00:00:02.947568 cr4=0000000000000000 ldtr=0000 tr=0000
00:00:02.947569 dr[0]=0000000000000000 dr[1]=0000000000000000 dr[2]=0000000000000000
00:00:02.947569 dr[3]=0000000000000000 dr[6]=0000000000000000 dr[7]=0000000000000000
00:00:02.947570 gdtr=0000000000000000:0000  idtr=0000000000000000:0000
00:00:02.947571 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.947572 FSbase=0000000000000000 GSbase=0000000000000000 efer=00000000
00:00:02.947600 Host CPUM state:
00:00:02.947601 rax=xxxxxxxxxxxxxxxx rbx=0000000000000000 rcx=xxxxxxxxxxxxxxxx
00:00:02.947602 rdx=xxxxxxxxxxxxxxxx rsi=0000000000000000 rdi=0000000000000000
00:00:02.947602 rip=xxxxxxxxxxxxxxxx rsp=0000000000000000 rbp=0000000000000000
00:00:02.947603  r8=xxxxxxxxxxxxxxxx  r9=xxxxxxxxxxxxxxxx r10=0000000000000000
00:00:02.947603 r11=0000000000000000 r12=0000000000000000 r13=0000000000000000
00:00:02.947604 r14=0000000000000000 r15=0000000000000000
00:00:02.947605 iopl=0   iopl=0 nv up di pl nz na pe nc
00:00:02.947605 cs=0000  ds=0000  es=0000  fs=0000  gs=0000                   eflags=00000000
00:00:02.947606 cr0=0000000000000000 cr2=xxxxxxxxxxxxxxxx cr3=0000000000000000
00:00:02.947607 cr4=0000000000000000 ldtr=0000 tr=0000
00:00:02.947608 dr[0]=0000000000000000 dr[1]=0000000000000000 dr[2]=0000000000000000
00:00:02.947608 dr[3]=0000000000000000 dr[6]=0000000000000000 dr[7]=0000000000000000
00:00:02.947609 gdtr=0000000000000000:0000  idtr=0000000000000000:0000
00:00:02.947610 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.947610 FSbase=0000000000000000 GSbase=0000000000000000 efer=00000000
00:00:02.947625 Host CPUM state:
00:00:02.947627 rax=xxxxxxxxxxxxxxxx rbx=0000000000000000 rcx=xxxxxxxxxxxxxxxx
00:00:02.947628 rdx=xxxxxxxxxxxxxxxx rsi=0000000000000000 rdi=0000000000000000
00:00:02.947628 rip=xxxxxxxxxxxxxxxx rsp=0000000000000000 rbp=0000000000000000
00:00:02.947629  r8=xxxxxxxxxxxxxxxx  r9=xxxxxxxxxxxxxxxx r10=0000000000000000
00:00:02.947629 r11=0000000000000000 r12=0000000000000000 r13=0000000000000000
00:00:02.947630 r14=0000000000000000 r15=0000000000000000
00:00:02.947630 iopl=0   iopl=0 nv up di pl nz na pe nc
00:00:02.947631 cs=0000  ds=0000  es=0000  fs=0000  gs=0000                   eflags=00000000
00:00:02.947632 cr0=0000000000000000 cr2=xxxxxxxxxxxxxxxx cr3=0000000000000000
00:00:02.947632 cr4=0000000000000000 ldtr=0000 tr=0000
00:00:02.947633 dr[0]=0000000000000000 dr[1]=0000000000000000 dr[2]=0000000000000000
00:00:02.947633 dr[3]=0000000000000000 dr[6]=0000000000000000 dr[7]=0000000000000000
00:00:02.947634 gdtr=0000000000000000:0000  idtr=0000000000000000:0000
00:00:02.947635 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.947635 FSbase=0000000000000000 GSbase=0000000000000000 efer=00000000
00:00:02.947646 !!
00:00:02.947646 !! {mode, all}
00:00:02.947647 !!
00:00:02.947651 Guest paging mode (VCPU #0):  Protected (changed 778 times), A20 enabled (changed 2 times)
00:00:02.947654 Guest SLAT mode (VCPU #0): Direct
00:00:02.947655 Shadow paging mode (VCPU #0): EPT
00:00:02.947656 Host paging mode:             AMD64+G+NX
00:00:02.947657 Guest paging mode (VCPU #1):  Real (changed 1 times), A20 enabled (changed 0 times)
00:00:02.947660 Guest SLAT mode (VCPU #1): Direct
00:00:02.947660 Shadow paging mode (VCPU #1): EPT
00:00:02.947661 Host paging mode:             AMD64+G+NX
00:00:02.947670 Guest paging mode (VCPU #2):  Real (changed 1 times), A20 enabled (changed 0 times)
00:00:02.947672 Guest SLAT mode (VCPU #2): Direct
00:00:02.947673 Shadow paging mode (VCPU #2): EPT
00:00:02.947674 Host paging mode:             AMD64+G+NX
00:00:02.947694 Guest paging mode (VCPU #3):  Real (changed 1 times), A20 enabled (changed 0 times)
00:00:02.947697 Guest SLAT mode (VCPU #3): Direct
00:00:02.947698 Shadow paging mode (VCPU #3): EPT
00:00:02.947698 Host paging mode:             AMD64+G+NX
00:00:02.947704 !!
00:00:02.947704 !! {cpuid, verbose}
00:00:02.947705 !!
00:00:02.947917          Raw Standard CPUID Leaves
00:00:02.947917      Leaf/sub-leaf  eax      ebx      ecx      edx
00:00:02.947918 Gst: 00000000/0000  0000000d 756e6547 6c65746e 49656e69
00:00:02.947920 Hst:                0000000d 756e6547 6c65746e 49656e69
00:00:02.947922 Gst: 00000001/0000  000306a9 00040800 769a2203 178bfbbf
00:00:02.947924 Hst:                000306a9 06100800 7fbae3ff bfebfbff
00:00:02.947926 Gst: 00000002/0000  76035a01 00f0b2ff 00000000 00ca0000
00:00:02.947927 Hst:                76035a01 00f0b2ff 00000000 00ca0000
00:00:02.947929 Gst: 00000003/0000  00000000 00000000 00000000 00000000
00:00:02.947930 Hst:                00000000 00000000 00000000 00000000
00:00:02.947932 Gst: 00000004/0000  0c000121 01c0003f 0000003f 00000000
00:00:02.947933 Hst:                1c004121 01c0003f 0000003f 00000000
00:00:02.947935 Gst: 00000004/0001  0c000122 01c0003f 0000003f 00000000
00:00:02.947936 Hst:                1c004122 01c0003f 0000003f 00000000
00:00:02.947938 Gst: 00000004/0002  0c000143 01c0003f 000001ff 00000000
00:00:02.947939 Hst:                1c004143 01c0003f 000001ff 00000000
00:00:02.947941 Gst: 00000004/0003  0c000163 03c0003f 00001fff 00000006
00:00:02.947942 Hst:                1c03c163 03c0003f 00001fff 00000006
00:00:02.947944 Gst: 00000004/0004  0c000000 00000000 00000000 00000000
00:00:02.947945 Hst:                00000000 00000000 00000000 00000000
00:00:02.947946 Gst: 00000005/0000  00000000 00000000 00000000 00000000
00:00:02.947947 Hst:                00000040 00000040 00000003 00001120
00:00:02.947949 Gst: 00000006/0000  00000004 00000000 00000000 00000000
00:00:02.947950 Hst:                00000077 00000002 00000009 00000000
00:00:02.947951 Gst: 00000007/0000  00000000 00000001 00000000 10000400
00:00:02.947953 Hst:                00000000 00000281 00000000 9c000400
00:00:02.947954 Gst: 00000007/0001  00000000 00000000 00000000 00000000
00:00:02.947955 Hst:                00000000 00000000 00000000 00000000
00:00:02.947956 Gst: 00000007/0002  00000000 00000000 00000000 00000000
00:00:02.947958 Hst:                00000000 00000000 00000000 00000000
00:00:02.947959 Gst: 00000008/0000  00000000 00000000 00000000 00000000
00:00:02.947960 Hst:                00000000 00000000 00000000 00000000
00:00:02.947961 Gst: 00000009/0000  00000000 00000000 00000000 00000000
00:00:02.947963 Hst:                00000000 00000000 00000000 00000000
00:00:02.947964 Gst: 0000000a/0000  00000000 00000000 00000000 00000000
00:00:02.947965 Hst:                07300403 00000000 00000000 00000603
00:00:02.947966 Gst: 0000000b/0000  00000000 00000001 00000100 00000000
00:00:02.947967 Hst:                00000001 00000002 00000100 00000006
00:00:02.947969 Gst: 0000000b/0001  00000002 00000004 00000201 00000000
00:00:02.947970 Hst:                00000004 00000008 00000201 00000006
00:00:02.947971 Gst: 0000000b/0002  00000000 00000000 00000002 00000000
00:00:02.947972 Hst:                00000000 00000000 00000002 00000006
00:00:02.947973 Gst: 0000000c/0000  00000000 00000000 00000000 00000000
00:00:02.947975 Hst:                00000000 00000000 00000000 00000000
00:00:02.947976 Gst: 0000000d/0000  00000007 00000340 00000340 00000000
00:00:02.947977 Hst:                00000007 00000340 00000340 00000000
00:00:02.947978 Gst: 0000000d/0001  00000000 00000000 00000000 00000000
00:00:02.947980 Hst:                00000001 00000000 00000000 00000000
00:00:02.947981 Gst: 0000000d/0002  00000100 00000240 00000000 00000000
00:00:02.947982 Hst:                00000100 00000240 00000000 00000000
00:00:02.947983 Gst: 0000000d/0003  00000000 00000000 00000000 00000000
00:00:02.947985 Hst:                00000000 00000000 00000000 00000000
00:00:02.947986                                Name: GenuineIntel
00:00:02.947987                            Supports: 0x00000000-0x0000000d
00:00:02.947990                              Family:  6 	Extended: 0 	Effective: 6
00:00:02.947991                               Model: 10 	Extended: 3 	Effective: 58
00:00:02.947992                            Stepping: 9
00:00:02.947993                                Type: 0 (primary)
00:00:02.947994                             APIC ID: 0x00
00:00:02.947995                        Logical CPUs: 4
00:00:02.947996                        CLFLUSH Size: 8
00:00:02.947997                            Brand ID: 0x00
00:00:02.948001 Features
00:00:02.948002   Mnemonic - Description                                  = Guest (Host)
00:00:02.948004   FPU - x87 FPU on Chip                                   = 1 (1)
00:00:02.948007   VME - Virtual 8086 Mode Enhancements                    = 1 (1)
00:00:02.948008   DE - Debugging extensions                               = 1 (1)
00:00:02.948010   PSE - Page Size Extension                               = 1 (1)
00:00:02.948012   TSC - Time Stamp Counter                                = 1 (1)
00:00:02.948013   MSR - Model Specific Registers                          = 1 (1)
00:00:02.948015   PAE - Physical Address Extension                        = 0 (1)
00:00:02.948017   MCE - Machine Check Exception                           = 1 (1)
00:00:02.948018   CX8 - CMPXCHG8B instruction                             = 1 (1)
00:00:02.948020   APIC - APIC On-Chip                                     = 1 (1)
00:00:02.948022   SEP - SYSENTER and SYSEXIT Present                      = 1 (1)
00:00:02.948024   MTRR - Memory Type Range Registers                      = 1 (1)
00:00:02.948025   PGE - PTE Global Bit                                    = 1 (1)
00:00:02.948027   MCA - Machine Check Architecture                        = 1 (1)
00:00:02.948028   CMOV - Conditional Move instructions                    = 1 (1)
00:00:02.948030   PAT - Page Attribute Table                              = 1 (1)
00:00:02.948032   PSE-36 - 36-bit Page Size Extension                     = 1 (1)
00:00:02.948033   PSN - Processor Serial Number                           = 0 (0)
00:00:02.948035   CLFSH - CLFLUSH instruction                             = 1 (1)
00:00:02.948036   DS - Debug Store                                        = 0 (1)
00:00:02.948038   ACPI - Thermal Mon. & Soft. Clock Ctrl.                 = 0 (1)
00:00:02.948040   MMX - Intel MMX Technology                              = 1 (1)
00:00:02.948041   FXSR - FXSAVE and FXRSTOR instructions                  = 1 (1)
00:00:02.948043   SSE - SSE support                                       = 1 (1)
00:00:02.948045   SSE2 - SSE2 support                                     = 1 (1)
00:00:02.948047   SS - Self Snoop                                         = 0 (1)
00:00:02.948049   HTT - Hyper-Threading Technology                        = 1 (1)
00:00:02.948050   TM - Therm. Monitor                                     = 0 (1)
00:00:02.948052   PBE - Pending Break Enabled                             = 0 (1)
00:00:02.948054   SSE3 - SSE3 support                                     = 1 (1)
00:00:02.948056   PCLMUL - PCLMULQDQ support (for AES-GCM)                = 1 (1)
00:00:02.948057   DTES64 - DS Area 64-bit Layout                          = 0 (1)
00:00:02.948059   MONITOR - MONITOR/MWAIT instructions                    = 0 (1)
00:00:02.948060   CPL-DS - CPL Qualified Debug Store                      = 0 (1)
00:00:02.948062   VMX - Virtual Machine Extensions                        = 0 (1)
00:00:02.948063   SMX - Safer Mode Extensions                             = 0 (1)
00:00:02.948065   EST - Enhanced SpeedStep Technology                     = 0 (1)
00:00:02.948066   TM2 - Terminal Monitor 2                                = 0 (1)
00:00:02.948068   SSSE3 - Supplemental Streaming SIMD Extensions 3        = 1 (1)
00:00:02.948069   CNTX-ID - L1 Context ID                                 = 0 (0)
00:00:02.948071   SDBG - Silicon Debug interface                          = 0 (0)
00:00:02.948073   FMA - Fused Multiply Add extensions                     = 0 (0)
00:00:02.948074   CX16 - CMPXCHG16B instruction                           = 1 (1)
00:00:02.948076   TPRUPDATE - xTPR Update Control                         = 0 (1)
00:00:02.948077   PDCM - Perf/Debug Capability MSR                        = 0 (1)
00:00:02.948079   PCID - Process Context Identifiers                      = 1 (1)
00:00:02.948080   DCA - Direct Cache Access                               = 0 (0)
00:00:02.948082   SSE4_1 - SSE4_1 support                                 = 1 (1)
00:00:02.948084   SSE4_2 - SSE4_2 support                                 = 1 (1)
00:00:02.948085   X2APIC - x2APIC support                                 = 0 (1)
00:00:02.948087   MOVBE - MOVBE instruction                               = 0 (0)
00:00:02.948089   POPCNT - POPCNT instruction                             = 1 (1)
00:00:02.948091   TSCDEADL - Time Stamp Counter Deadline                  = 0 (1)
00:00:02.948092   AES - AES instructions                                  = 1 (1)
00:00:02.948094   XSAVE - XSAVE instruction                               = 1 (1)
00:00:02.948096   OSXSAVE - OSXSAVE instruction                           = 0 (1)
00:00:02.948097   AVX - AVX support                                       = 1 (1)
00:00:02.948099   F16C - 16-bit floating point conversion instructions    = 1 (1)
00:00:02.948100   RDRAND - RDRAND instruction                             = 1 (1)
00:00:02.948102   HVP - Hypervisor Present (we're a guest)                = 0 (0)
00:00:02.948103 Structured Extended Feature Flags Enumeration (leaf 7):
00:00:02.948104 Sub-leaf 0
00:00:02.948104   Mnemonic - Description                                  = Guest (Host)
00:00:02.948106   FSGSBASE - RDFSBASE/RDGSBASE/WRFSBASE/WRGSBASE instr.   = 1 (1)
00:00:02.948107   TSCADJUST - Supports MSR_IA32_TSC_ADJUST                = 0 (0)
00:00:02.948109   SGX - Supports Software Guard Extensions                = 0 (0)
00:00:02.948110   BMI1 - Advanced Bit Manipulation extension 1            = 0 (0)
00:00:02.948111   HLE - Hardware Lock Elision                             = 0 (0)
00:00:02.948113   AVX2 - Advanced Vector Extensions 2                     = 0 (0)
00:00:02.948115   FDP_EXCPTN_ONLY - FPU DP only updated on exceptions     = 0 (0)
00:00:02.948116   SMEP - Supervisor Mode Execution Prevention             = 0 (1)
00:00:02.948117   BMI2 - Advanced Bit Manipulation extension 2            = 0 (0)
00:00:02.948118   ERMS - Enhanced REP MOVSB/STOSB instructions            = 0 (1)
00:00:02.948120   INVPCID - INVPCID instruction                           = 0 (0)
00:00:02.948121   RTM - Restricted Transactional Memory                   = 0 (0)
00:00:02.948123   PQM - Platform Quality of Service Monitoring            = 0 (0)
00:00:02.948124   DEPFPU_CS_DS - Deprecates FPU CS, FPU DS values if set  = 0 (0)
00:00:02.948125   MPE - Intel Memory Protection Extensions                = 0 (0)
00:00:02.948126   PQE - Platform Quality of Service Enforcement           = 0 (0)
00:00:02.948128   AVX512F - AVX512 Foundation instructions                = 0 (0)
00:00:02.948129   AVX512DQ - Supports the AVX512DQ instructions           = 0 (0)
00:00:02.948130   RDSEED - RDSEED instruction                             = 0 (0)
00:00:02.948132   ADX - ADCX/ADOX instructions                            = 0 (0)
00:00:02.948134   SMAP - Supervisor Mode Access Prevention                = 0 (0)
00:00:02.948135   AVX512_IFMA - Supports the AVX512_IFMA instructions     = 0 (0)
00:00:02.948136   CLFLUSHOPT - CLFLUSHOPT (Cache Line Flush) instruction  = 0 (0)
00:00:02.948137   CLWB - CLWB instruction                                 = 0 (0)
00:00:02.948139   INTEL_PT - Intel Processor Trace                        = 0 (0)
00:00:02.948140   AVX512PF - AVX512 Prefetch instructions                 = 0 (0)
00:00:02.948142   AVX512ER - AVX512 Exponential & Reciprocal instructions = 0 (0)
00:00:02.948143   AVX512CD - AVX512 Conflict Detection instructions       = 0 (0)
00:00:02.948144   SHA - Secure Hash Algorithm extensions                  = 0 (0)
00:00:02.948145   AVX512BW - Supports the AVX512BW instructions           = 0 (0)
00:00:02.948147   AVX512VL - Supports the AVX512VL instructions           = 0 (0)
00:00:02.948148   PREFETCHWT1 - PREFETCHWT1 instruction                   = 0 (0)
00:00:02.948149   AVX512_VBMI - Supports the AVX512_VBMI instructions     = 0 (0)
00:00:02.948150   UMIP - User mode insturction prevention                 = 0 (0)
00:00:02.948152   PKU - Protection Key for Usermode pages                 = 0 (0)
00:00:02.948153   OSPKE - CR4.PKU mirror                                  = 0 (0)
00:00:02.948155   WAITPKG - TPAUSE, UMONITOR & UMWAIT support             = 0 (0)
00:00:02.948156   AVX512_VBMI2 - Supports the AVX512_VBMI2 instructions   = 0 (0)
00:00:02.948157   CET_SS - CET shadow stack support                       = 0 (0)
00:00:02.948159   GFNI - Supports the GFNI instruction set                = 0 (0)
00:00:02.948160   VAES - Supports the VEX encoded AES instruction set     = 0 (0)
00:00:02.948161   VPCLMULQDQ - Supports the VPCLMULQDQ instruction        = 0 (0)
00:00:02.948162   AVX512_VNNI - Supports the AVX512_VNNI instructions     = 0 (0)
00:00:02.948163   AVX512_BITALG - Supports the AVX512_BITALG instructions = 0 (0)
00:00:02.948165   TME_EN - Supports 4 IA32_TME_ MSRs                      = 0 (0)
00:00:02.948166   AVX512_VPOPCNTDQ - Supports the AVX512_VPOPCNTDQ instructions = 0 (0)
00:00:02.948167   LA57 - 57-bit linear addresses                          = 0 (0)
00:00:02.948169   MAWAU - Value used by BNDLDX & BNDSTX                   = 0x0 (0x0)
00:00:02.948170   RDPID - Read processor ID support                       = 0 (0)
00:00:02.948172   KEY_LOCKER - Supports Key Locker                        = 0 (0)
00:00:02.948174   BUS_LOCK_DETECT - Supports OS bus-lock detection        = 0 (0)
00:00:02.948175   CLDEMOTE - Supports cache line demote                   = 0 (0)
00:00:02.948176   MOVDIRI - Supports the MOVDIRI instruction              = 0 (0)
00:00:02.948177   MOVDIRI64B - Supports the MOVDIRI64B instruction        = 0 (0)
00:00:02.948179   ENQCMD - Supports the Eqnqueue Stores                   = 0 (0)
00:00:02.948180   SGX_LC - Supports SGX Launch Configuration              = 0 (0)
00:00:02.948181   PKS - Supports protection keys for supervisor pages     = 0 (0)
00:00:02.948186   SGX_KEYS - Supports Attestation Service for Intel SGX   = 0 (0)
00:00:02.948187   AVX512_4VNNIW - Supports the AVX512_4VNNIW instructions = 0 (0)
00:00:02.948188   AVX512_4FMAPS - Supports the AVX512_4FMAPS instructions = 0 (0)
00:00:02.948189   FAST_SHORT_REP_MOVSB - Supports fast short REP MOVSB    = 0 (0)
00:00:02.948190   UINTR - Supports user interrupts                        = 0 (0)
00:00:02.948191   AVX512_VP2INTERSECT - Supports the AVX512_VP2INTERSECT instr. = 0 (0)
00:00:02.948192   MCU_OPT_CTRL - Supports IA32_MCU_OPT_CTRL               = 0 (0)
00:00:02.948194   MD_CLEAR - Supports MDS related buffer clearing         = 1 (1)
00:00:02.948195   RTM_ALWAYS_ABORT - XBEGIN always aborts and does fallback = 0 (0)
00:00:02.948196   RTM_FORCE_ABORT - Supports IA32_TSX_FORCE_ABORT         = 0 (0)
00:00:02.948197   SERIALIZE - Supports the SERIALIZE instruction          = 0 (0)
00:00:02.948198   HYBRID - Identifiers the CPU as a hybrid part           = 0 (0)
00:00:02.948200   TSXLDTRK - Supports susp/resume of TSX ld addr tracking = 0 (0)
00:00:02.948201   PCONFIG - Supports the PCONFIG instruction              = 0 (0)
00:00:02.948202   ARCH_LBRS - Supports architectural LBRs                 = 0 (0)
00:00:02.948203   CET_IBT - Supports indirect branch tracking w/ CET      = 0 (0)
00:00:02.948204   AMX_BF16 - Supports tile comp. ops on bfloat16 number   = 0 (0)
00:00:02.948206   AVX512_FP16 - Supports the FP16 data type with AVX512   = 0 (0)
00:00:02.948207   AMX_TILE - Supports the tile architecture               = 0 (0)
00:00:02.948208   AMX_INT8 - Supports tile comp. ops on 8-bit integers    = 0 (0)
00:00:02.948209   IBRS_IBPB - IA32_SPEC_CTRL.IBRS and IA32_PRED_CMD.IBPB  = 0 (1)
00:00:02.948210   STIBP - Supports IA32_SPEC_CTRL.STIBP                   = 0 (1)
00:00:02.948211   FLUSH_CMD - Supports IA32_FLUSH_CMD                     = 1 (1)
00:00:02.948213   ARCHCAP - Supports IA32_ARCH_CAP                        = 0 (0)
00:00:02.948214   CORECAP - Supports IA32_CORE_CAP                        = 0 (0)
00:00:02.948216   SSBD - Supports IA32_SPEC_CTRL.SSBD                     = 0 (1)
00:00:02.948217  Sub-leaf 2
00:00:02.948218   Mnemonic - Description                                  = Guest (Host)
00:00:02.948219   PSFD - Supports IA32_SPEC_CTRL[7] (PSFD)                = 0 (0)
00:00:02.948220   IPRED_CTRL - Supports IA32_SPEC_CTRL[4:3] (IPRED_DIS)   = 0 (0)
00:00:02.948222   RRSBA_CTRL - Supports IA32_SPEC_CTRL[6:5] (RRSBA_DIS)   = 0 (0)
00:00:02.948223   DDPD_U - Supports IA32_SPEC_CTRL[8] (DDPD_U)            = 0 (0)
00:00:02.948224   BHI_CTRL - Supports IA32_SPEC_CTRL[10] (BHI_DIS_S)      = 0 (0)
00:00:02.948225   MCDT_NO - No MXCSR Config Dependent Timing issues       = 0 (0)
00:00:02.948226   UC_LOCK_DIS - Supports UC-lock disable and causing #AC  = 0 (0)
00:00:02.948227   MONITOR_MITG_NO - No MONITOR/UMONITOR power issues      = 0 (0)
00:00:02.948228 Processor Extended State Enumeration (leaf 0xd):
00:00:02.948229    XSAVE area cur/max size by XCR0, Guest: 0x340/0x340
00:00:02.948230    XSAVE area cur/max size by XCR0,  Host: 0x340/0x340
00:00:02.948232                    Valid XCR0 bits, Guest: 0x00000000`00000007 ( x87 SSE YMM_Hi128 )
00:00:02.948235                    Valid XCR0 bits,  Host: 0x00000000`00000007 ( x87 SSE YMM_Hi128 )
00:00:02.948238                     XSAVE features, Guest
00:00:02.948239                     XSAVE features,  Host XSAVEOPT
00:00:02.948241       XSAVE area cur size XCR0|XSS, Guest: 0x0
00:00:02.948242       XSAVE area cur size XCR0|XSS,  Host: 0x0
00:00:02.948243                Valid IA32_XSS bits, Guest: 0x00000000`00000000
00:00:02.948244                Valid IA32_XSS bits,  Host: 0x00000000`00000000
00:00:02.948246   State #2, Guest: off=0x0240, cb=0x0100 IA32_XSS-bit -- YMM_Hi128
00:00:02.948248   State #2,  Host:  off=0x0240, cb=0x0100 IA32_XSS-bit -- YMM_Hi128
00:00:02.948253          Raw Extended CPUID Leaves
00:00:02.948253      Leaf/sub-leaf  eax      ebx      ecx      edx
00:00:02.948254 Gst: 80000000/0000  80000008 00000000 00000000 00000000
00:00:02.948255 Hst:                80000008 00000000 00000000 00000000
00:00:02.948257 Gst: 80000001/0000  00000000 00000000 00000001 08000800
00:00:02.948258 Hst:                00000000 00000000 00000001 28100800
00:00:02.948260 Gst: 80000002/0000  20202020 20202020 65746e49 2952286c
00:00:02.948262 Hst:                20202020 20202020 65746e49 2952286c
00:00:02.948263 Gst: 80000003/0000  726f4320 4d542865 37692029 3737332d
00:00:02.948265 Hst:                726f4320 4d542865 37692029 3737332d
00:00:02.948267 Gst: 80000004/0000  50432030 20402055 30342e33 007a4847
00:00:02.948269 Hst:                50432030 20402055 30342e33 007a4847
00:00:02.948270 Gst: 80000005/0000  00000000 00000000 00000000 00000000
00:00:02.948272 Hst:                00000000 00000000 00000000 00000000
00:00:02.948273 Gst: 80000006/0000  00000000 00000000 01006040 00000000
00:00:02.948275 Hst:                00000000 00000000 01006040 00000000
00:00:02.948276 Gst: 80000007/0000  00000000 00000000 00000000 00000100
00:00:02.948277 Hst:                00000000 00000000 00000000 00000100
00:00:02.948278 Gst: 80000008/0000  00003024 00000000 00000000 00000000
00:00:02.948280 Hst:                00003024 00000000 00000000 00000000
00:00:02.948281 Ext Name:
00:00:02.948282 Ext Supports:                    0x80000000-0x80000008
00:00:02.948283 Family:                          0  	Extended: 0 	Effective: 0
00:00:02.948283 Model:                           0  	Extended: 0 	Effective: 0
00:00:02.948284 Stepping:                        0
00:00:02.948284 Brand ID:                        0x000
00:00:02.948286 Ext Features
00:00:02.948286   Mnemonic - Description                                  = Guest (Host)
00:00:02.948288   FPU - x87 FPU on Chip                                   = 0 (0)
00:00:02.948290   VME - Virtual 8086 Mode Enhancements                    = 0 (0)
00:00:02.948292   DE - Debugging extensions                               = 0 (0)
00:00:02.948294   PSE - Page Size Extension                               = 0 (0)
00:00:02.948295   TSC - Time Stamp Counter                                = 0 (0)
00:00:02.948297   MSR - K86 Model Specific Registers                      = 0 (0)
00:00:02.948298   PAE - Physical Address Extension                        = 0 (0)
00:00:02.948300   MCE - Machine Check Exception                           = 0 (0)
00:00:02.948301   CX8 - CMPXCHG8B instruction                             = 0 (0)
00:00:02.948303   APIC - APIC On-Chip                                     = 0 (0)
00:00:02.948305   SEP - SYSCALL/SYSRET                                    = 1 (1)
00:00:02.948307   MTRR - Memory Type Range Registers                      = 0 (0)
00:00:02.948308   PGE - PTE Global Bit                                    = 0 (0)
00:00:02.948310   MCA - Machine Check Architecture                        = 0 (0)
00:00:02.948312   CMOV - Conditional Move instructions                    = 0 (0)
00:00:02.948313   PAT - Page Attribute Table                              = 0 (0)
00:00:02.948315   PSE-36 - 36-bit Page Size Extension                     = 0 (0)
00:00:02.948317   NX - No-Execute/Execute-Disable                         = 0 (1)
00:00:02.948318   AXMMX - AMD Extensions to MMX instructions              = 0 (0)
00:00:02.948319   MMX - Intel MMX Technology                              = 0 (0)
00:00:02.948326   FXSR - FXSAVE and FXRSTOR Instructions                  = 0 (0)
00:00:02.948327   FFXSR - AMD fast FXSAVE and FXRSTOR instructions        = 0 (0)
00:00:02.948328   Page1GB - 1 GB large page                               = 0 (0)
00:00:02.948330   RDTSCP - RDTSCP instruction                             = 1 (1)
00:00:02.948332   LM - AMD64 Long Mode                                    = 0 (1)
00:00:02.948334   3DNOWEXT - AMD Extensions to 3DNow                      = 0 (0)
00:00:02.948335   3DNOW - AMD 3DNow                                       = 0 (0)
00:00:02.948337   LahfSahf - LAHF/SAHF support in 64-bit mode             = 1 (1)
00:00:02.948338   CmpLegacy - Core multi-processing legacy mode           = 0 (0)
00:00:02.948340   SVM - AMD Secure Virtual Machine extensions             = 0 (0)
00:00:02.948341   EXTAPIC - AMD Extended APIC registers                   = 0 (0)
00:00:02.948342   CR8L - AMD LOCK MOV CR0 means MOV CR8                   = 0 (0)
00:00:02.948344   ABM - AMD Advanced Bit Manipulation                     = 0 (0)
00:00:02.948345   SSE4A - SSE4A instructions                              = 0 (0)
00:00:02.948347   MISALIGNSSE - AMD Misaligned SSE mode                   = 0 (0)
00:00:02.948348   3DNOWPRF - AMD PREFETCH and PREFETCHW instructions      = 0 (0)
00:00:02.948349   OSVW - AMD OS Visible Workaround                        = 0 (0)
00:00:02.948351   IBS - Instruct Based Sampling                           = 0 (0)
00:00:02.948352   XOP - Extended Operation support                        = 0 (0)
00:00:02.948354   SKINIT - SKINIT, STGI, and DEV support                  = 0 (0)
00:00:02.948355   WDT - AMD Watchdog Timer support                        = 0 (0)
00:00:02.948357   LWP - Lightweight Profiling support                     = 0 (0)
00:00:02.948358   FMA4 - Four operand FMA instruction support             = 0 (0)
00:00:02.948360   TCE - Translation Cache Extension support               = 0 (0)
00:00:02.948361   NodeId - NodeId in MSR C001_100C                        = 0 (0)
00:00:02.948363   TBM - Trailing Bit Manipulation instructions            = 0 (0)
00:00:02.948364   TOPOEXT - Topology Extensions                           = 0 (0)
00:00:02.948365   PRFEXTCORE - Performance Counter Extensions support     = 0 (0)
00:00:02.948367   PRFEXTNB - NB Performance Counter Extensions support    = 0 (0)
00:00:02.948368   DATABPEXT - Data-access Breakpoint Extension            = 0 (0)
00:00:02.948369   PERFTSC - Performance Time Stamp Counter                = 0 (0)
00:00:02.948370   PCX_L2I - L2I/L3 Performance Counter Extensions         = 0 (0)
00:00:02.948371   MONITORX - MWAITX and MONITORX instructions             = 0 (0)
00:00:02.948373   AddrMaskExt - BP Addressing masking extended to bit 31  = 0 (0)
00:00:02.948374 Full Name:                       "        Intel(R) Core(TM) i7-3770 CPU @ 3.40GHz"
00:00:02.948375 TLB 2/4M Instr/Uni:              res0     0 entries
00:00:02.948375 TLB 2/4M Data:                   res0     0 entries
00:00:02.948376 TLB 4K Instr/Uni:                res0     0 entries
00:00:02.948377 TLB 4K Data:                     res0     0 entries
00:00:02.948378 L1 Instr Cache Line Size:        0 bytes
00:00:02.948378 L1 Instr Cache Lines Per Tag:    0
00:00:02.948378 L1 Instr Cache Associativity:    res0
00:00:02.948379 L1 Instr Cache Size:             0 KB
00:00:02.948380 L1 Data Cache Line Size:         0 bytes
00:00:02.948380 L1 Data Cache Lines Per Tag:     0
00:00:02.948380 L1 Data Cache Associativity:     res0
00:00:02.948381 L1 Data Cache Size:              0 KB
00:00:02.948382 L2 TLB 2/4M Instr/Uni:           off       0 entries
00:00:02.948382 L2 TLB 2/4M Data:                off       0 entries
00:00:02.948383 L2 TLB 4K Instr/Uni:             off       0 entries
00:00:02.948383 L2 TLB 4K Data:                  off       0 entries
00:00:02.948384 L2 Cache Line Size:              64 bytes
00:00:02.948385 L2 Cache Lines Per Tag:          0
00:00:02.948385 L2 Cache Associativity:          8 way
00:00:02.948385 L2 Cache Size:                   256 KB
00:00:02.948389 L3 Cache Line Size:              0 bytes
00:00:02.948389 L3 Cache Lines Per Tag:          0
00:00:02.948389 L3 Cache Associativity:          off
00:00:02.948390 L3 Cache Size:                   0 KB
00:00:02.948391 APM Features EDX
00:00:02.948391   Mnemonic - Description                                  = Guest (Host)
00:00:02.948393   TS - Temperature Sensor                                 = 0 (0)
00:00:02.948395   FID - Frequency ID control                              = 0 (0)
00:00:02.948397   VID - Voltage ID control                                = 0 (0)
00:00:02.948398   TTP - Thermal Trip                                      = 0 (0)
00:00:02.948400   TM - Hardware Thermal Control (HTC)                     = 0 (0)
00:00:02.948402   100MHzSteps - 100 MHz Multiplier control                = 0 (0)
00:00:02.948403   HwPstate - Hardware P-state control                     = 0 (0)
00:00:02.948404   TscInvariant - Invariant Time Stamp Counter             = 1 (1)
00:00:02.948406   CPB - Core Performance Boost                            = 0 (0)
00:00:02.948407   EffFreqRO - Read-only Effective Frequency Interface     = 0 (0)
00:00:02.948409   ProcFdbkIf - Processor Feedback Interface               = 0 (0)
00:00:02.948410   ProcPwrRep - Core power reporting interface support     = 0 (0)
00:00:02.948411   ConnectedStandby - Connected Standby                    = 0 (0)
00:00:02.948422   RAPL - Running average power limit                      = 0 (0)
00:00:02.948424 Physical Address Width:          36 bits
00:00:02.948424 Virtual Address Width:           48 bits
00:00:02.948425 Max page count for INVLPGB:      0x3024
00:00:02.948426 Max ECX for RDPRU:               0x0
00:00:02.948427 !!
00:00:02.948427 !! {handlers, phys virt hyper stats}
00:00:02.948428 !!
00:00:02.948430 Physical handlers: max 0x1800, 0 allocator error, 0 tree error
00:00:02.948431 From             - To (incl)         Handler (R3)      uUser             Type     Description
00:00:02.948434 00000000000a0000 - 00000000000bffff  00007fd543aff3c0  0000000000000002  MMIO     VGA - VGA Video Buffer  (r0-enabled)
00:00:02.948438 00000000000c0000 - 00000000000c8fff  00007fd543b215c0  0000000000000004  Write    VGA BIOS  (r0-enabled)
00:00:02.948441 00000000000e0000 - 00000000000e0fff  00007fd543b215c0  0000000000000005  Write    ACPI RSDP  (r0-enabled)
00:00:02.948443 00000000000e1000 - 00000000000e1fff  00007fd543b215c0  0000000000000000  Write    DMI tables  (r0-enabled)
00:00:02.948446 00000000000e2000 - 00000000000effff  00007fd543b215c0  0000000000000003  Write    Net Boot ROM  (r0-enabled)
00:00:02.948448 00000000000f0000 - 00000000000fffff  00007fd543b215c0  0000000000000001  Write    PC BIOS - 0xfffff  (r0-enabled)
00:00:02.948451 00000000e0000000 - 00000000e7ffffff  00007fd543b1e6c0  0000000000000003  Write    VRam  (keep-pgm-lock, r0-enabled)
00:00:02.948454 00000000f0000000 - 00000000f0000fff  00007fd543aff3c0  0000000000000003  MMIO     PCnet  (r0-enabled)
00:00:02.948457 00000000f0804000 - 00000000f0804fff  00007fd543aff3c0  0000000000000005  MMIO     USB OHCI  (r0-enabled)
00:00:02.948459 00000000f0805000 - 00000000f0805fff  00007fd543aff3c0  0000000000000004  MMIO     USB EHCI  (r0-enabled)
00:00:02.948462 00000000fec00000 - 00000000fec00fff  00007fd543aff3c0  0000000000000001  MMIO     I/O APIC  (r0-enabled)
00:00:02.948465 00000000fee00000 - 00000000fee00fff  00007fd543aff3c0  0000000000000000  MMIO     APIC  (r0-enabled)
00:00:02.948467 00000000ffff0000 - 00000000ffffffff  00007fd543b215c0  0000000000000002  Write    PC BIOS - 0xffffffff  (r0-enabled)
00:00:02.948470 !!
00:00:02.948470 !! {timers, <NULL>}
00:00:02.948471 !!
00:00:02.948473 Timers (pVM=00007fd542fc9000)
00:00:02.948474 pTimerR3         offNext  offPrev  offSched Clock               Time             Expire HzHint State                     Description
00:00:02.948478 00007fd556a1d080 ffffffff ffffffff ffffffff Real             1901911                  0      0 1-STOPPED                 BlkCache-Commit
00:00:02.948484 00007fd556a1d100 ffffffff ffffffff ffffffff Real             1901911                  0      0 1-STOPPED                 PS2M Throttle
00:00:02.948491 00007fd556a1d180 00000007 ffffffff ffffffff Real             1901911            1901913      0 2-ACTIVE                  VGA Refresh
00:00:02.948495 00007fd556a1d200 ffffffff ffffffff ffffffff Real             1901911                  0      0 1-STOPPED                 AudioEnum-0
00:00:02.948500 00007fd556a1d280 ffffffff ffffffff ffffffff Real             1901911                  0      0 1-STOPPED                 AudioEnum-1[1]
00:00:02.948504 00007fd556a1d300 ffffffff ffffffff ffffffff Real             1901911                  0      0 1-STOPPED                 AudioEnum-2[2]
00:00:02.948508 00007fd556a1d380 ffffffff 00000003 ffffffff Real             1901911            1902137      0 2-ACTIVE                  CPU Load Timer
00:00:02.948513 00007fd550018080 ffffffff ffffffff ffffffff Virt          2788069119           28521400      0 1-STOPPED                 PS2K Throttle
00:00:02.948518 00007fd550018100 ffffffff ffffffff ffffffff Virt          2788069119                  0      0 1-STOPPED                 PS2K Typematic
00:00:02.948522 00007fd550018180 ffffffff ffffffff ffffffff Virt          2788069119           14910959      0 1-STOPPED                 PS2K Delay
00:00:02.948527 00007fd550018200 ffffffff ffffffff ffffffff Virt          2788069119                  0      0 1-STOPPED                 PS2M Delay
00:00:02.948531 00007fd550018280 ffffffff ffffffff ffffffff Virt          2788069119                  0      0 1-STOPPED                 Heartbeat flatlined
00:00:02.948535 00007fd550018300 ffffffff ffffffff ffffffff Virt          2788069119                  0      0 1-STOPPED                 PCnet Poll
00:00:02.948539 00007fd550018380 ffffffff ffffffff ffffffff Virt          2788069119                  0      0 1-STOPPED                 PCnet SoftInt
00:00:02.948543 00007fd550018400 ffffffff ffffffff ffffffff Virt          2788069119                  0      0 1-STOPPED                 PCnet Restore
00:00:02.948549 00007fd550fea080 ffffffff ffffffff ffffffff VrSy          2788019477                  0      0 1-STOPPED                 APIC Timer 0
00:00:02.948554 00007fd550fea100 ffffffff ffffffff ffffffff VrSy          2788019477                  0      0 1-STOPPED                 APIC Timer 1
00:00:02.948558 00007fd550fea180 ffffffff ffffffff ffffffff VrSy          2788019477                  0      0 1-STOPPED                 APIC Timer 2
00:00:02.948562 00007fd550fea200 ffffffff ffffffff ffffffff VrSy          2788019477                  0      0 1-STOPPED                 APIC Timer 3
00:00:02.948566 00007fd550fea280 00000007 ffffffff ffffffff VrSy          2788019477         2822941917     18 2-ACTIVE                  i8254 PIT
00:00:02.948571 00007fd550fea300 ffffffff ffffffff ffffffff VrSy          2788019477                  0      0 1-STOPPED                 MC146818 RTC Periodic
00:00:02.948575 00007fd550fea380 0000000c 00000005 ffffffff VrSy          2788019477         2990000000      0 2-ACTIVE                  MC146818 RTC Second
00:00:02.948579 00007fd550fea400 ffffffff ffffffff ffffffff VrSy          2788019477         1990244140      0 1-STOPPED                 MC146818 RTC Second2
00:00:02.948584 00007fd550fea480 ffffffff ffffffff ffffffff VrSy          2788019477                  0      0 1-STOPPED                 AC97 PI
00:00:02.948588 00007fd550fea500 ffffffff ffffffff ffffffff VrSy          2788019477                  0      0 1-STOPPED                 AC97 PO
00:00:02.948592 00007fd550fea580 ffffffff ffffffff ffffffff VrSy          2788019477                  0      0 1-STOPPED                 AC97 MC
00:00:02.948596 00007fd550fea600 ffffffff 00000007 ffffffff VrSy          2788019477       599932015941      0 2-ACTIVE                  ACPI PM
00:00:02.948602 !!
00:00:02.948602 !! {activetimers, <NULL>}
00:00:02.948602 !!
00:00:02.948604 Active Timers (pVM=00007fd542fc9000)
00:00:02.948605 pTimerR3         offNext  offPrev  offSched Clock               Time             Expire HzHint State                     Description
00:00:02.948610 00007fd556a1d180 00000007 ffffffff ffffffff Real             1901911            1901913      0 2-ACTIVE                  VGA Refresh
00:00:02.948615 00007fd556a1d380 ffffffff 00000003 ffffffff Real             1901911            1902137      0 2-ACTIVE                  CPU Load Timer
00:00:02.948619 00007fd550fea280 00000007 ffffffff ffffffff VrSy          2788019477         2822941917     18 2-ACTIVE                  i8254 PIT
00:00:02.948624 00007fd550fea380 0000000c 00000005 ffffffff VrSy          2788019477         2990000000      0 2-ACTIVE                  MC146818 RTC Second
00:00:02.948628 00007fd550fea600 ffffffff 00000007 ffffffff VrSy          2788019477       599932015941      0 2-ACTIVE                  ACPI PM
00:00:02.948634 !!
00:00:02.948635 !! {ac97bdl}
00:00:02.948635 !!
00:00:02.948636 BDL for stream #0: @ 0x0 LB 0x100; CIV=0x00 LVI=0x00:
00:00:02.948638 BDL for stream #1: @ 0x0 LB 0x100; CIV=0x00 LVI=0x00:
00:00:02.948640 BDL for stream #2: @ 0x0 LB 0x100; CIV=0x00 LVI=0x00:
00:00:02.948641 !!
00:00:02.948641 !! {ac97mixer}
00:00:02.948642 !!
00:00:02.948643 [Master]   AC'97 Mixer: fMuted=true  auChannels=ff ff ff ff ff ff ff ff ff ff ff ff
00:00:02.948648 [Sink 0]       Line In: fMuted=true  auChannels=ff ff ff ff ff ff ff ff ff ff ff ff
00:00:02.948652 [Sink 1] Microphone In: fMuted=true  auChannels=ff ff ff ff ff ff ff ff ff ff ff ff
00:00:02.948655 [Sink 2]    PCM Output: fMuted=true  auChannels=ff ff ff ff ff ff ff ff ff ff ff ff
00:00:02.948658 !!
00:00:02.948659 !! {ac97stream}
00:00:02.948659 !!
00:00:02.948662 Stream #0: '' invalid 0ch U0 0Hz, 0ms buffer, 0ms period, 0ms pre-buffer, 0ms sched, invalid
00:00:02.948663   BDBAR   0x00000000
00:00:02.948664   CIV     0x00
00:00:02.948665   LVI     0x00
00:00:02.948666   SR      0x0000
00:00:02.948667   PICB    0x0000
00:00:02.948667   PIV     0x00
00:00:02.948668   CR      0x00
00:00:02.948669   offRead            0x0
00:00:02.948670   offWrite           0x0
00:00:02.948670   uTimerHz           0
00:00:02.948671   cDmaPeriodTicks    0
00:00:02.948672   cbDmaPeriod        0x0
00:00:02.948674 Stream #1: '' invalid 0ch U0 0Hz, 0ms buffer, 0ms period, 0ms pre-buffer, 0ms sched, invalid
00:00:02.948675   BDBAR   0x00000000
00:00:02.948676   CIV     0x00
00:00:02.948676   LVI     0x00
00:00:02.948677   SR      0x0000
00:00:02.948678   PICB    0x0000
00:00:02.948678   PIV     0x00
00:00:02.948679   CR      0x00
00:00:02.948680   offRead            0x0
00:00:02.948681   offWrite           0x0
00:00:02.948681   uTimerHz           0
00:00:02.948682   cDmaPeriodTicks    0
00:00:02.948683   cbDmaPeriod        0x0
00:00:02.948685 Stream #2: '' invalid 0ch U0 0Hz, 0ms buffer, 0ms period, 0ms pre-buffer, 0ms sched, invalid
00:00:02.948685   BDBAR   0x00000000
00:00:02.948686   CIV     0x00
00:00:02.948687   LVI     0x00
00:00:02.948687   SR      0x0000
00:00:02.948688   PICB    0x0000
00:00:02.948689   PIV     0x00
00:00:02.948689   CR      0x00
00:00:02.948690   offRead            0x0
00:00:02.948691   offWrite           0x0
00:00:02.948691   uTimerHz           0
00:00:02.948692   cDmaPeriodTicks    0
00:00:02.948693   cbDmaPeriod        0x0
00:00:02.948694 !!
00:00:02.948694 !! {acpi}
00:00:02.948694 !!
00:00:02.948695 timer: old=00000000, current=00000000
00:00:02.948697 !!
00:00:02.948697 !! {apic}
00:00:02.948697 !!
00:00:02.948699 APIC0:
00:00:02.948700   APIC Base MSR                 = 0xfee00900 (Addr=0xfee00000 en bsp)
00:00:02.948702   Mode                          = 2 (xAPIC)
00:00:02.948703   APIC ID                       = 0 (0x0)
00:00:02.948704   Version                       = 0x50014
00:00:02.948704     APIC Version                  = 0x14
00:00:02.948705     Max LVT entry index (0..N)    = 5
00:00:02.948706     EOI Broadcast supression      = false
00:00:02.948707   APR                           = 0 (0x0)
00:00:02.948707   TPR                           = 0 (0x0)
00:00:02.948708     Task-priority class           = 0x0
00:00:02.948709     Task-priority subclass        = 0x0
00:00:02.948711   PPR                           = 0 (0x0)
00:00:02.948712     Processor-priority class      = 0x0
00:00:02.948713     Processor-priority subclass   = 0x0
00:00:02.948713   RRD                           = 0 (0x0)
00:00:02.948714   LDR                           = 0x0
00:00:02.948714     Logical APIC ID               = 0x0
00:00:02.948715   DFR                           = 0xffffffff
00:00:02.948716     Model                         = 0xf (Flat)
00:00:02.948717   SVR                           = 0x10f
00:00:02.948717     Vector                        = 15 (0xf)
00:00:02.948718     Software Enabled              = true
00:00:02.948719     Supress EOI broadcast         = false
00:00:02.948720   ISR
00:00:02.948720     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948723     Pending: None
00:00:02.948724   TMR
00:00:02.948724     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948726     Pending: None
00:00:02.948727   IRR
00:00:02.948728     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948730     Pending: None
00:00:02.948731   ESR                           = 0x0
00:00:02.948731     Redirectable IPI              = false
00:00:02.948732     Send Illegal Vector           = false
00:00:02.948733     Recv Illegal Vector           = false
00:00:02.948733     Illegal Register Address      = false
00:00:02.948734   ICR Low                       = 0x0
00:00:02.948735     Vector                        = 0 (0x0)
00:00:02.948736     Delivery Mode                 = 0x0 (Fixed)
00:00:02.948737     Destination Mode              = 0x0 (Physical)
00:00:02.948737     Delivery Status               = 0
00:00:02.948738     Level                         = 0
00:00:02.948738     Trigger Mode                  = 0 (Edge)
00:00:02.948739     Destination shorthand         = 0x0 (None)
00:00:02.948740   ICR High                      = 0x0
00:00:02.948740     Destination field/mask        = 0x0
00:00:02.948741   ESR Internal                  = 0x0
00:00:02.948742   PIB
00:00:02.948742     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948744     Pending: None
00:00:02.948745   Level PIB
00:00:02.948745     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948747     Pending: None
00:00:02.948749 APIC1:
00:00:02.948750   APIC Base MSR                 = 0xfee00800 (Addr=0xfee00000 en)
00:00:02.948752   Mode                          = 2 (xAPIC)
00:00:02.948753   APIC ID                       = 1 (0x1)
00:00:02.948754   Version                       = 0x50014
00:00:02.948755     APIC Version                  = 0x14
00:00:02.948755     Max LVT entry index (0..N)    = 5
00:00:02.948756     EOI Broadcast supression      = false
00:00:02.948757   APR                           = 0 (0x0)
00:00:02.948758   TPR                           = 0 (0x0)
00:00:02.948759     Task-priority class           = 0x0
00:00:02.948759     Task-priority subclass        = 0x0
00:00:02.948760   PPR                           = 0 (0x0)
00:00:02.948760     Processor-priority class      = 0x0
00:00:02.948761     Processor-priority subclass   = 0x0
00:00:02.948762   RRD                           = 0 (0x0)
00:00:02.948763   LDR                           = 0x0
00:00:02.948763     Logical APIC ID               = 0x0
00:00:02.948764   DFR                           = 0xffffffff
00:00:02.948765     Model                         = 0xf (Flat)
00:00:02.948765   SVR                           = 0xff
00:00:02.948766     Vector                        = 255 (0xff)
00:00:02.948767     Software Enabled              = false
00:00:02.948768     Supress EOI broadcast         = false
00:00:02.948768   ISR
00:00:02.948769     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948772     Pending: None
00:00:02.948772   TMR
00:00:02.948773     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948775     Pending: None
00:00:02.948776   IRR
00:00:02.948776     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948783     Pending: None
00:00:02.948784   ESR                           = 0x0
00:00:02.948784     Redirectable IPI              = false
00:00:02.948785     Send Illegal Vector           = false
00:00:02.948786     Recv Illegal Vector           = false
00:00:02.948786     Illegal Register Address      = false
00:00:02.948787   ICR Low                       = 0x0
00:00:02.948788     Vector                        = 0 (0x0)
00:00:02.948789     Delivery Mode                 = 0x0 (Fixed)
00:00:02.948790     Destination Mode              = 0x0 (Physical)
00:00:02.948790     Delivery Status               = 0
00:00:02.948791     Level                         = 0
00:00:02.948792     Trigger Mode                  = 0 (Edge)
00:00:02.948792     Destination shorthand         = 0x0 (None)
00:00:02.948793   ICR High                      = 0x0
00:00:02.948794     Destination field/mask        = 0x0
00:00:02.948794   ESR Internal                  = 0x0
00:00:02.948795   PIB
00:00:02.948795     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948798     Pending: None
00:00:02.948798   Level PIB
00:00:02.948799     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948801     Pending: None
00:00:02.948808 APIC2:
00:00:02.948809   APIC Base MSR                 = 0xfee00800 (Addr=0xfee00000 en)
00:00:02.948811   Mode                          = 2 (xAPIC)
00:00:02.948811   APIC ID                       = 2 (0x2)
00:00:02.948812   Version                       = 0x50014
00:00:02.948813     APIC Version                  = 0x14
00:00:02.948814     Max LVT entry index (0..N)    = 5
00:00:02.948814     EOI Broadcast supression      = false
00:00:02.948815   APR                           = 0 (0x0)
00:00:02.948816   TPR                           = 0 (0x0)
00:00:02.948816     Task-priority class           = 0x0
00:00:02.948817     Task-priority subclass        = 0x0
00:00:02.948817   PPR                           = 0 (0x0)
00:00:02.948818     Processor-priority class      = 0x0
00:00:02.948819     Processor-priority subclass   = 0x0
00:00:02.948819   RRD                           = 0 (0x0)
00:00:02.948820   LDR                           = 0x0
00:00:02.948821     Logical APIC ID               = 0x0
00:00:02.948821   DFR                           = 0xffffffff
00:00:02.948822     Model                         = 0xf (Flat)
00:00:02.948823   SVR                           = 0xff
00:00:02.948823     Vector                        = 255 (0xff)
00:00:02.948824     Software Enabled              = false
00:00:02.948825     Supress EOI broadcast         = false
00:00:02.948826   ISR
00:00:02.948826     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948828     Pending: None
00:00:02.948829   TMR
00:00:02.948829     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948831     Pending: None
00:00:02.948832   IRR
00:00:02.948832     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948835     Pending: None
00:00:02.948835   ESR                           = 0x0
00:00:02.948836     Redirectable IPI              = false
00:00:02.948837     Send Illegal Vector           = false
00:00:02.948837     Recv Illegal Vector           = false
00:00:02.948838     Illegal Register Address      = false
00:00:02.948839   ICR Low                       = 0x0
00:00:02.948839     Vector                        = 0 (0x0)
00:00:02.948840     Delivery Mode                 = 0x0 (Fixed)
00:00:02.948841     Destination Mode              = 0x0 (Physical)
00:00:02.948842     Delivery Status               = 0
00:00:02.948842     Level                         = 0
00:00:02.948843     Trigger Mode                  = 0 (Edge)
00:00:02.948843     Destination shorthand         = 0x0 (None)
00:00:02.948844   ICR High                      = 0x0
00:00:02.948845     Destination field/mask        = 0x0
00:00:02.948845   ESR Internal                  = 0x0
00:00:02.948846   PIB
00:00:02.948846     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948849     Pending: None
00:00:02.948852   Level PIB
00:00:02.948853     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948855     Pending: None
00:00:02.948875 APIC3:
00:00:02.948876   APIC Base MSR                 = 0xfee00800 (Addr=0xfee00000 en)
00:00:02.948879   Mode                          = 2 (xAPIC)
00:00:02.948880   APIC ID                       = 3 (0x3)
00:00:02.948880   Version                       = 0x50014
00:00:02.948881     APIC Version                  = 0x14
00:00:02.948882     Max LVT entry index (0..N)    = 5
00:00:02.948882     EOI Broadcast supression      = false
00:00:02.948883   APR                           = 0 (0x0)
00:00:02.948883   TPR                           = 0 (0x0)
00:00:02.948884     Task-priority class           = 0x0
00:00:02.948884     Task-priority subclass        = 0x0
00:00:02.948885   PPR                           = 0 (0x0)
00:00:02.948885     Processor-priority class      = 0x0
00:00:02.948885     Processor-priority subclass   = 0x0
00:00:02.948886   RRD                           = 0 (0x0)
00:00:02.948886   LDR                           = 0x0
00:00:02.948887     Logical APIC ID               = 0x0
00:00:02.948888   DFR                           = 0xffffffff
00:00:02.948889     Model                         = 0xf (Flat)
00:00:02.948889   SVR                           = 0xff
00:00:02.948890     Vector                        = 255 (0xff)
00:00:02.948891     Software Enabled              = false
00:00:02.948892     Supress EOI broadcast         = false
00:00:02.948893   ISR
00:00:02.948893     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948896     Pending: None
00:00:02.948896   TMR
00:00:02.948897     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948900     Pending: None
00:00:02.948900   IRR
00:00:02.948901     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948903     Pending: None
00:00:02.948904   ESR                           = 0x0
00:00:02.948905     Redirectable IPI              = false
00:00:02.948905     Send Illegal Vector           = false
00:00:02.948906     Recv Illegal Vector           = false
00:00:02.948907     Illegal Register Address      = false
00:00:02.948908   ICR Low                       = 0x0
00:00:02.948909     Vector                        = 0 (0x0)
00:00:02.948910     Delivery Mode                 = 0x0 (Fixed)
00:00:02.948910     Destination Mode              = 0x0 (Physical)
00:00:02.948911     Delivery Status               = 0
00:00:02.948912     Level                         = 0
00:00:02.948913     Trigger Mode                  = 0 (Edge)
00:00:02.948914     Destination shorthand         = 0x0 (None)
00:00:02.948915   ICR High                      = 0x0
00:00:02.948915     Destination field/mask        = 0x0
00:00:02.948916   ESR Internal                  = 0x0
00:00:02.948917   PIB
00:00:02.948917     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948920     Pending: None
00:00:02.948921   Level PIB
00:00:02.948921     0000000000000000000000000000000000000000000000000000000000000000
00:00:02.948923     Pending: None
00:00:02.948933 !!
00:00:02.948934 !! {apiclvt}
00:00:02.948934 !!
00:00:02.948936 VCPU[0] APIC Local Vector Table (LVT):
00:00:02.948937 lvt     timermode  mask  trigger  rirr  polarity  dlvr_st  dlvr_mode   vector
00:00:02.948938 Timer    One-shot   1                               Idle                 0 (0x0)
00:00:02.948941 Thermal             1                               Idle     Fixed       0 (0x0)
00:00:02.948943 Perf                1                               Idle     Fixed       0 (0x0)
00:00:02.948946 LINT0               0     Edge      0   ActiveHi    Idle     ExtINT      0 (0x0)
00:00:02.948948 LINT1               0     Edge      0   ActiveHi    Idle     NMI         0 (0x0)
00:00:02.948961 Error               1                               Idle     Fixed       0 (0x0)
00:00:02.948964 VCPU[1] APIC Local Vector Table (LVT):
00:00:02.948965 lvt     timermode  mask  trigger  rirr  polarity  dlvr_st  dlvr_mode   vector
00:00:02.948968 Timer    One-shot   1                               Idle                 0 (0x0)
00:00:02.948971 Thermal             1                               Idle     Fixed       0 (0x0)
00:00:02.948973 Perf                1                               Idle     Fixed       0 (0x0)
00:00:02.948975 LINT0               1     Edge      0   ActiveHi    Idle     Fixed       0 (0x0)
00:00:02.948978 LINT1               1     Edge      0   ActiveHi    Idle     Fixed       0 (0x0)
00:00:02.948979 Error               1                               Idle     Fixed       0 (0x0)
00:00:02.948995 VCPU[2] APIC Local Vector Table (LVT):
00:00:02.948996 lvt     timermode  mask  trigger  rirr  polarity  dlvr_st  dlvr_mode   vector
00:00:02.948997 Timer    One-shot   1                               Idle                 0 (0x0)
00:00:02.948999 Thermal             1                               Idle     Fixed       0 (0x0)
00:00:02.949001 Perf                1                               Idle     Fixed       0 (0x0)
00:00:02.949003 LINT0               1     Edge      0   ActiveHi    Idle     Fixed       0 (0x0)
00:00:02.949005 LINT1               1     Edge      0   ActiveHi    Idle     Fixed       0 (0x0)
00:00:02.949006 Error               1                               Idle     Fixed       0 (0x0)
00:00:02.949021 VCPU[3] APIC Local Vector Table (LVT):
00:00:02.949022 lvt     timermode  mask  trigger  rirr  polarity  dlvr_st  dlvr_mode   vector
00:00:02.949023 Timer    One-shot   1                               Idle                 0 (0x0)
00:00:02.949026 Thermal             1                               Idle     Fixed       0 (0x0)
00:00:02.949028 Perf                1                               Idle     Fixed       0 (0x0)
00:00:02.949031 LINT0               1     Edge      0   ActiveHi    Idle     Fixed       0 (0x0)
00:00:02.949033 LINT1               1     Edge      0   ActiveHi    Idle     Fixed       0 (0x0)
00:00:02.949036 Error               1                               Idle     Fixed       0 (0x0)
00:00:02.949047 !!
00:00:02.949048 !! {apictimer}
00:00:02.949048 !!
00:00:02.949050 VCPU[0] Local APIC timer:
00:00:02.949051   ICR              = 0x0
00:00:02.949052   CCR              = 0x0
00:00:02.949053   DCR              = 0x0
00:00:02.949053     Timer shift    = 0x1
00:00:02.949054 LVT Timer          = 0x10000
00:00:02.949055   Vector             = 0 (0x0)
00:00:02.949056   Delivery status    = 0
00:00:02.949056   Masked             = true
00:00:02.949057   Timer Mode         = 0x0 (One-shot)
00:00:02.949058   Timer initial TS = 0
00:00:02.949059 VCPU[1] Local APIC timer:
00:00:02.949060   ICR              = 0x0
00:00:02.949061   CCR              = 0x0
00:00:02.949062   DCR              = 0x0
00:00:02.949063     Timer shift    = 0x1
00:00:02.949063 LVT Timer          = 0x10000
00:00:02.949064   Vector             = 0 (0x0)
00:00:02.949065   Delivery status    = 0
00:00:02.949066   Masked             = true
00:00:02.949066   Timer Mode         = 0x0 (One-shot)
00:00:02.949067   Timer initial TS = 0
00:00:02.949074 VCPU[2] Local APIC timer:
00:00:02.949074   ICR              = 0x0
00:00:02.949075   CCR              = 0x0
00:00:02.949076   DCR              = 0x0
00:00:02.949077     Timer shift    = 0x1
00:00:02.949077 LVT Timer          = 0x10000
00:00:02.949078   Vector             = 0 (0x0)
00:00:02.949079   Delivery status    = 0
00:00:02.949079   Masked             = true
00:00:02.949080   Timer Mode         = 0x0 (One-shot)
00:00:02.949081   Timer initial TS = 0
00:00:02.949092 VCPU[3] Local APIC timer:
00:00:02.949094   ICR              = 0x0
00:00:02.949095   CCR              = 0x0
00:00:02.949096   DCR              = 0x0
00:00:02.949096     Timer shift    = 0x1
00:00:02.949097 LVT Timer          = 0x10000
00:00:02.949098   Vector             = 0 (0x0)
00:00:02.949099   Delivery status    = 0
00:00:02.949099   Masked             = true
00:00:02.949100   Timer Mode         = 0x0 (One-shot)
00:00:02.949100   Timer initial TS = 0
00:00:02.949108 !!
00:00:02.949109 !! {bugcheck}
00:00:02.949112 !!
00:00:02.949113 No bug check reported.
00:00:02.949114 !!
00:00:02.949114 !! {cfgm}
00:00:02.949114 !!
00:00:02.949115 pRoot=00007fd538001010:{/}
00:00:02.949117 [/] (level 0)
00:00:02.949119   CpuExecutionCap   <integer> = 0x0000000000000064 (100)
00:00:02.949121   EnablePAE         <integer> = 0x0000000000000000 (0)
00:00:02.949123   HMEnabled         <integer> = 0x0000000000000001 (1)
00:00:02.949125   MemBalloonSize    <integer> = 0x0000000000000000 (0, 0 B)
00:00:02.949127   Name              <string>  = "poler-os64-minimal" (cb=19)
00:00:02.949129   NumCPUs           <integer> = 0x0000000000000004 (4)
00:00:02.949130   PageFusionAllowed <integer> = 0x0000000000000000 (0)
00:00:02.949131   RamHoleSize       <integer> = 0x0000000020000000 (536 870 912, 512.0 MiB)
00:00:02.949134   RamSize           <integer> = 0x0000000100000000 (4 294 967 296, 4.0 GiB)
00:00:02.949137   TimerMillies      <integer> = 0x000000000000000a (10)
00:00:02.949138   UUID              <bytes>   = "6f f8 c8 df 9e a1 af 4d 9b 7c 7d bb d7 f0 9e e8" (cb=16)
00:00:02.949143
00:00:02.949143 [/CPUM/] (level 1)
00:00:02.949145   Enable64bit        <integer> = 0x0000000000000000 (0)
00:00:02.949146   GuestCpuName       <string>  = "host" (cb=5)
00:00:02.949147   NestedHWVirt       <integer> = 0x0000000000000000 (0)
00:00:02.949148   PortableCpuIdLevel <integer> = 0x0000000000000000 (0)
00:00:02.949150   SpecCtrl           <integer> = 0x0000000000000000 (0)
00:00:02.949151
00:00:02.949151 [/CPUM/IsaExts/] (level 2)
00:00:02.949153
00:00:02.949153 [/DBGC/] (level 1)
00:00:02.949154   GlobalInitScript <string>  = "/home/vitalij/.config/VirtualBox/dbgc-init" (cb=43)
00:00:02.949156   HistoryFile      <string>  = "/home/vitalij/.config/VirtualBox/dbgc-history" (cb=46)
00:00:02.949157   LocalInitScript  <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/dbgc-init" (cb=58)
00:00:02.949158
00:00:02.949158 [/DBGF/] (level 1)
00:00:02.949159   Path <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/debug/;/home/vitalij/VirtualBox VMs/poler-os64-minimal/;cache*/home/vitalij/VirtualBox VMs/poler-os64-minimal/dbgcache/;/home/vitalij/" (cb=183)
00:00:02.949161
00:00:02.949161 [/Devices/] (level 1)
00:00:02.949162
00:00:02.949163 [/Devices/3c501/] (level 2)
00:00:02.949164
00:00:02.949165 [/Devices/8237A/] (level 2)
00:00:02.949166
00:00:02.949166 [/Devices/8237A/0/] (level 3)
00:00:02.949168   Trusted <integer> = 0x0000000000000001 (1)
00:00:02.949169
00:00:02.949169 [/Devices/8237A/0/Config/] (level 4) (restricted root)
00:00:02.949172
00:00:02.949172 [/Devices/VMMDev/] (level 2)
00:00:02.949173
00:00:02.949174 [/Devices/VMMDev/0/] (level 3)
00:00:02.949176   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:02.949177   PCIDeviceNo   <integer> = 0x0000000000000004 (4)
00:00:02.949178   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:02.949179   Trusted       <integer> = 0x0000000000000001 (1)
00:00:02.949180
00:00:02.949181 [/Devices/VMMDev/0/Config/] (level 4) (restricted root)
00:00:02.949183   GuestCoreDumpDir <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/Snapshots" (cb=58)
00:00:02.949184
00:00:02.949184 [/Devices/VMMDev/0/LUN#0/] (level 4)
00:00:02.949186   Driver <string>  = "HGCM" (cb=5)
00:00:02.949187
00:00:02.949187 [/Devices/VMMDev/0/LUN#0/Config/] (level 5) (restricted root)
00:00:02.949189
00:00:02.949189 [/Devices/VMMDev/0/LUN#999/] (level 4)
00:00:02.949191   Driver <string>  = "MainStatus" (cb=11)
00:00:02.949192
00:00:02.949192 [/Devices/VMMDev/0/LUN#999/Config/] (level 5) (restricted root)
00:00:02.949194   First                <integer> = 0x0000000000000000 (0)
00:00:02.949196   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:02.949197   Last                 <integer> = 0x0000000000000000 (0)
00:00:02.949199   iLedSet              <integer> = 0x0000000000000005 (5)
00:00:02.949200
00:00:02.949201 [/Devices/acpi/] (level 2)
00:00:02.949202
00:00:02.949202 [/Devices/acpi/0/] (level 3)
00:00:02.949204   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:02.949207   PCIDeviceNo   <integer> = 0x0000000000000007 (7)
00:00:02.949208   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:02.949210   Trusted       <integer> = 0x0000000000000001 (1)
00:00:02.949211
00:00:02.949211 [/Devices/acpi/0/Config/] (level 4) (restricted root)
00:00:02.949213   CpuHotPlug          <integer> = 0x0000000000000000 (0)
00:00:02.949215   FdcEnabled          <integer> = 0x0000000000000000 (0)
00:00:02.949216   HostBusPciAddress   <integer> = 0x0000000000000000 (0)
00:00:02.949217   HpetEnabled         <integer> = 0x0000000000000000 (0)
00:00:02.949218   IOAPIC              <integer> = 0x0000000000000001 (1)
00:00:02.949220   IocPciAddress       <integer> = 0x0000000000010000 (65 536)
00:00:02.949222   NumCPUs             <integer> = 0x0000000000000004 (4)
00:00:02.949223   Parallel0IoPortBase <integer> = 0x0000000000000000 (0)
00:00:02.949224   Parallel0Irq        <integer> = 0x0000000000000000 (0)
00:00:02.949225   Parallel1IoPortBase <integer> = 0x0000000000000000 (0)
00:00:02.949227   Parallel1Irq        <integer> = 0x0000000000000000 (0)
00:00:02.949228   Serial0IoPortBase   <integer> = 0x0000000000000000 (0)
00:00:02.949229   Serial0Irq          <integer> = 0x0000000000000000 (0)
00:00:02.949230   Serial1IoPortBase   <integer> = 0x0000000000000000 (0)
00:00:02.949231   Serial1Irq          <integer> = 0x0000000000000000 (0)
00:00:02.949233   ShowCpu             <integer> = 0x0000000000000001 (1)
00:00:02.949234   ShowRtc             <integer> = 0x0000000000000000 (0)
00:00:02.949235   SmcEnabled          <integer> = 0x0000000000000000 (0)
00:00:02.949236
00:00:02.949237 [/Devices/acpi/0/LUN#0/] (level 4)
00:00:02.949239   Driver <string>  = "ACPIHost" (cb=9)
00:00:02.949240
00:00:02.949240 [/Devices/acpi/0/LUN#0/Config/] (level 5) (restricted root)
00:00:02.949242
00:00:02.949242 [/Devices/acpi/0/LUN#1/] (level 4)
00:00:02.949244   Driver <string>  = "ACPICpu" (cb=8)
00:00:02.949245
00:00:02.949245 [/Devices/acpi/0/LUN#1/Config/] (level 5)
00:00:02.949247
00:00:02.949247 [/Devices/acpi/0/LUN#2/] (level 4)
00:00:02.949249   Driver <string>  = "ACPICpu" (cb=8)
00:00:02.949250
00:00:02.949250 [/Devices/acpi/0/LUN#2/Config/] (level 5)
00:00:02.949252
00:00:02.949252 [/Devices/acpi/0/LUN#3/] (level 4)
00:00:02.949254   Driver <string>  = "ACPICpu" (cb=8)
00:00:02.949255
00:00:02.949255 [/Devices/acpi/0/LUN#3/Config/] (level 5)
00:00:02.949257
00:00:02.949257 [/Devices/apic/] (level 2)
00:00:02.949259
00:00:02.949259 [/Devices/apic/0/] (level 3)
00:00:02.949261   Trusted <integer> = 0x0000000000000001 (1)
00:00:02.949262
00:00:02.949262 [/Devices/apic/0/Config/] (level 4) (restricted root)
00:00:02.949264   IOAPIC  <integer> = 0x0000000000000001 (1)
00:00:02.949265   Mode    <integer> = 0x0000000000000002 (2)
00:00:02.949266   NumCPUs <integer> = 0x0000000000000004 (4)
00:00:02.949267
00:00:02.949268 [/Devices/dp8390/] (level 2)
00:00:02.949269
00:00:02.949270 [/Devices/e1000/] (level 2)
00:00:02.949271
00:00:02.949271 [/Devices/i8254/] (level 2)
00:00:02.949273
00:00:02.949273 [/Devices/i8254/0/] (level 3)
00:00:02.949274
00:00:02.949275 [/Devices/i8254/0/Config/] (level 4) (restricted root)
00:00:02.949276
00:00:02.949277 [/Devices/i8259/] (level 2)
00:00:02.949278
00:00:02.949278 [/Devices/i8259/0/] (level 3)
00:00:02.949280   Trusted <integer> = 0x0000000000000001 (1)
00:00:02.949281
00:00:02.949281 [/Devices/i8259/0/Config/] (level 4) (restricted root)
00:00:02.949283
00:00:02.949283 [/Devices/ichac97/] (level 2)
00:00:02.949285
00:00:02.949285 [/Devices/ichac97/0/] (level 3)
00:00:02.949287   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:02.949288   PCIDeviceNo   <integer> = 0x0000000000000005 (5)
00:00:02.949289   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:02.949290   Trusted       <integer> = 0x0000000000000001 (1)
00:00:02.949291
00:00:02.949292 [/Devices/ichac97/0/AudioConfig/] (level 4)
00:00:02.949293
00:00:02.949294 [/Devices/ichac97/0/Config/] (level 4) (restricted root)
00:00:02.949296   Codec        <string>  = "STAC9700" (cb=9)
00:00:02.949298   DebugEnabled <integer> = 0x0000000000000000 (0)
00:00:02.949300
00:00:02.949300 [/Devices/ichac97/0/LUN#0/] (level 4)
00:00:02.949302   Driver <string>  = "AUDIO" (cb=6)
00:00:02.949302
00:00:02.949303 [/Devices/ichac97/0/LUN#0/AttachedDriver/] (level 5)
00:00:02.949305   Driver <string>  = "ALSAAudio" (cb=10)
00:00:02.949306
00:00:02.949307 [/Devices/ichac97/0/LUN#0/AttachedDriver/Config/] (level 6) (restricted root)
00:00:02.949309
00:00:02.949309 [/Devices/ichac97/0/LUN#0/Config/] (level 5) (restricted root)
00:00:02.949311   DriverName    <string>  = "ALSAAudio" (cb=10)
00:00:02.949312   InputEnabled  <integer> = 0x0000000000000000 (0)
00:00:02.949314   OutputEnabled <integer> = 0x0000000000000001 (1)
00:00:02.949315
00:00:02.949315 [/Devices/ichac97/0/LUN#1/] (level 4)
00:00:02.949317   Driver <string>  = "AUDIO" (cb=6)
00:00:02.949318
00:00:02.949318 [/Devices/ichac97/0/LUN#1/Config/] (level 5) (restricted root)
00:00:02.949320
00:00:02.949320 [/Devices/ichac97/0/LUN#2/] (level 4)
00:00:02.949322   Driver <string>  = "AUDIO" (cb=6)
00:00:02.949323
00:00:02.949323 [/Devices/ichac97/0/LUN#2/Config/] (level 5) (restricted root)
00:00:02.949325
00:00:02.949326 [/Devices/ioapic/] (level 2)
00:00:02.949327
00:00:02.949327 [/Devices/ioapic/0/] (level 3)
00:00:02.949329   Trusted <integer> = 0x0000000000000001 (1)
00:00:02.949330
00:00:02.949330 [/Devices/ioapic/0/Config/] (level 4) (restricted root)
00:00:02.949332   NumCPUs <integer> = 0x0000000000000004 (4)
00:00:02.949333
00:00:02.949333 [/Devices/mc146818/] (level 2)
00:00:02.949335
00:00:02.949335 [/Devices/mc146818/0/] (level 3)
00:00:02.949336
00:00:02.949337 [/Devices/mc146818/0/Config/] (level 4) (restricted root)
00:00:02.949338   UseUTC <integer> = 0x0000000000000000 (0)
00:00:02.949340
00:00:02.949340 [/Devices/parallel/] (level 2)
00:00:02.949341
00:00:02.949341 [/Devices/pcarch/] (level 2)
00:00:02.949343
00:00:02.949343 [/Devices/pcarch/0/] (level 3)
00:00:02.949345   Trusted <integer> = 0x0000000000000001 (1)
00:00:02.949346
00:00:02.949346 [/Devices/pcarch/0/Config/] (level 4) (restricted root)
00:00:02.949348
00:00:02.949348 [/Devices/pcbios/] (level 2)
00:00:02.949350
00:00:02.949350 [/Devices/pcbios/0/] (level 3)
00:00:02.949351   Trusted <integer> = 0x0000000000000001 (1)
00:00:02.949352
00:00:02.949353 [/Devices/pcbios/0/Config/] (level 4) (restricted root)
00:00:02.949355   APIC            <integer> = 0x0000000000000001 (1)
00:00:02.949357   BootDevice0     <string>  = "FLOPPY" (cb=7)
00:00:02.949358   BootDevice1     <string>  = "DVD" (cb=4)
00:00:02.949359   BootDevice2     <string>  = "IDE" (cb=4)
00:00:02.949360   BootDevice3     <string>  = "NONE" (cb=5)
00:00:02.949360   DmiSystemSerial <string>  = "VirtualBox-<DmiSystemUuid>" (cb=27)
00:00:02.949361   FloppyDevice    <string>  = "i82078" (cb=7)
00:00:02.949362   HardDiskDevice  <string>  = "piix3ide" (cb=9)
00:00:02.949363   IOAPIC          <integer> = 0x0000000000000001 (1)
00:00:02.949365   McfgBase        <integer> = 0x0000000000000000 (0)
00:00:02.949366   McfgLength      <integer> = 0x0000000000000000 (0)
00:00:02.949367   NumCPUs         <integer> = 0x0000000000000004 (4)
00:00:02.949368   PXEDebug        <integer> = 0x0000000000000000 (0)
00:00:02.949370   UUID            <bytes>   = "6f f8 c8 df 9e a1 af 4d 9b 7c 7d bb d7 f0 9e e8" (cb=16)
00:00:02.949374   UuidLe          <integer> = 0x0000000000000001 (1)
00:00:02.949375
00:00:02.949375 [/Devices/pcbios/0/Config/NetBoot/] (level 5)
00:00:02.949377
00:00:02.949378 [/Devices/pcbios/0/Config/NetBoot/0/] (level 6)
00:00:02.949380   NIC           <integer> = 0x0000000000000000 (0)
00:00:02.949381   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:02.949382   PCIDeviceNo   <integer> = 0x0000000000000003 (3)
00:00:02.949384   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:02.949385
00:00:02.949385 [/Devices/pci/] (level 2)
00:00:02.949386
00:00:02.949387 [/Devices/pci/0/] (level 3)
00:00:02.949388   Trusted <integer> = 0x0000000000000001 (1)
00:00:02.949389
00:00:02.949390 [/Devices/pci/0/Config/] (level 4) (restricted root)
00:00:02.949393   IOAPIC <integer> = 0x0000000000000001 (1)
00:00:02.949394
00:00:02.949395 [/Devices/pcibridge/] (level 2)
00:00:02.949396
00:00:02.949396 [/Devices/pckbd/] (level 2)
00:00:02.949398
00:00:02.949398 [/Devices/pckbd/0/] (level 3)
00:00:02.949399   Trusted <integer> = 0x0000000000000001 (1)
00:00:02.949400
00:00:02.949401 [/Devices/pckbd/0/Config/] (level 4) (restricted root)
00:00:02.949402
00:00:02.949403 [/Devices/pckbd/0/LUN#0/] (level 4)
00:00:02.949404   Driver <string>  = "KeyboardQueue" (cb=14)
00:00:02.949405
00:00:02.949406 [/Devices/pckbd/0/LUN#0/AttachedDriver/] (level 5)
00:00:02.949408   Driver <string>  = "MainKeyboard" (cb=13)
00:00:02.949409
00:00:02.949409 [/Devices/pckbd/0/LUN#0/AttachedDriver/Config/] (level 6) (restricted root)
00:00:02.949411
00:00:02.949421 [/Devices/pckbd/0/LUN#0/Config/] (level 5) (restricted root)
00:00:02.949423   QueueSize <integer> = 0x0000000000000040 (64, 64 B)
00:00:02.949425
00:00:02.949425 [/Devices/pckbd/0/LUN#1/] (level 4)
00:00:02.949427   Driver <string>  = "MouseQueue" (cb=11)
00:00:02.949428
00:00:02.949428 [/Devices/pckbd/0/LUN#1/AttachedDriver/] (level 5)
00:00:02.949430   Driver <string>  = "MainMouse" (cb=10)
00:00:02.949431
00:00:02.949431 [/Devices/pckbd/0/LUN#1/AttachedDriver/Config/] (level 6) (restricted root)
00:00:02.949433
00:00:02.949434 [/Devices/pckbd/0/LUN#1/Config/] (level 5) (restricted root)
00:00:02.949436   QueueSize <integer> = 0x0000000000000080 (128, 128 B)
00:00:02.949437
00:00:02.949438 [/Devices/pcnet/] (level 2)
00:00:02.949439
00:00:02.949440 [/Devices/pcnet/0/] (level 3)
00:00:02.949441   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:02.949442   PCIDeviceNo   <integer> = 0x0000000000000003 (3)
00:00:02.949444   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:02.949445   Trusted       <integer> = 0x0000000000000001 (1)
00:00:02.949446
00:00:02.949446 [/Devices/pcnet/0/Config/] (level 4) (restricted root)
00:00:02.949448   CableConnected <integer> = 0x0000000000000001 (1)
00:00:02.949449   ChipType       <string>  = "Am79C973" (cb=9)
00:00:02.949450   LineSpeed      <integer> = 0x0000000000000000 (0)
00:00:02.949451   MAC            <bytes>   = "08 00 27 62 22 16" (cb=6)
00:00:02.949454
00:00:02.949454 [/Devices/pcnet/0/LUN#0/] (level 4)
00:00:02.949456   Driver <string>  = "NAT" (cb=4)
00:00:02.949457
00:00:02.949457 [/Devices/pcnet/0/LUN#0/Config/] (level 5) (restricted root)
00:00:02.949459   AliasMode          <integer> = 0x0000000000000000 (0)
00:00:02.949460   DNSProxy           <integer> = 0x0000000000000000 (0)
00:00:02.949462   EnableTFTP         <integer> = 0x0000000000000000 (0)
00:00:02.949463   ForwardBroadcast   <integer> = 0x0000000000000000 (0)
00:00:02.949464   LocalhostReachable <integer> = 0x0000000000000001 (1)
00:00:02.949465   Network            <string>  = "10.0.2.0/24" (cb=12)
00:00:02.949467   PassDomain         <integer> = 0x0000000000000001 (1)
00:00:02.949468   UseHostResolver    <integer> = 0x0000000000000000 (0)
00:00:02.949469
00:00:02.949469 [/Devices/pcnet/0/LUN#999/] (level 4)
00:00:02.949471   Driver <string>  = "MainStatus" (cb=11)
00:00:02.949472
00:00:02.949472 [/Devices/pcnet/0/LUN#999/Config/] (level 5) (restricted root)
00:00:02.949474   First                <integer> = 0x0000000000000000 (0)
00:00:02.949476   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:02.949477   Last                 <integer> = 0x0000000000000000 (0)
00:00:02.949478   iLedSet              <integer> = 0x0000000000000004 (4)
00:00:02.949480
00:00:02.949480 [/Devices/piix3ide/] (level 2)
00:00:02.949482
00:00:02.949482 [/Devices/piix3ide/0/] (level 3)
00:00:02.949484   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:02.949485   PCIDeviceNo   <integer> = 0x0000000000000001 (1)
00:00:02.949486   PCIFunctionNo <integer> = 0x0000000000000001 (1)
00:00:02.949487   Trusted       <integer> = 0x0000000000000001 (1)
00:00:02.949488
00:00:02.949489 [/Devices/piix3ide/0/Config/] (level 4) (restricted root)
00:00:02.949490   Type <string>  = "PIIX4" (cb=6)
00:00:02.949493
00:00:02.949494 [/Devices/piix3ide/0/LUN#0/] (level 4)
00:00:02.949496   Driver <string>  = "VD" (cb=3)
00:00:02.949496
00:00:02.949497 [/Devices/piix3ide/0/LUN#0/Config/] (level 5) (restricted root)
00:00:02.949499   Format    <string>  = "VDI" (cb=4)
00:00:02.949500   Mountable <integer> = 0x0000000000000000 (0)
00:00:02.949501   Path      <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/poler-os64-minimal.vdi" (cb=71)
00:00:02.949502   Type      <string>  = "HardDisk" (cb=9)
00:00:02.949503
00:00:02.949504 [/Devices/piix3ide/0/LUN#0/Config/VDConfig/] (level 6)
00:00:02.949506   AllocationBlockSize <string>  = "1048576" (cb=8)
00:00:02.949507
00:00:02.949507 [/Devices/piix3ide/0/LUN#2/] (level 4)
00:00:02.949509   Driver <string>  = "VD" (cb=3)
00:00:02.949510
00:00:02.949510 [/Devices/piix3ide/0/LUN#2/Config/] (level 5) (restricted root)
00:00:02.949512   Format    <string>  = "RAW" (cb=4)
00:00:02.949513   Mountable <integer> = 0x0000000000000001 (1)
00:00:02.949514   Path      <string>  = "/home/vitalij/Стільниця/разроботка/Нова тека/ZCodeProject/poler-os-work/poler-os64-minimal.iso" (cb=122)
00:00:02.949515   ReadOnly  <integer> = 0x0000000000000001 (1)
00:00:02.949517   Type      <string>  = "DVD" (cb=4)
00:00:02.949518
00:00:02.949518 [/Devices/piix3ide/0/LUN#999/] (level 4)
00:00:02.949520   Driver <string>  = "MainStatus" (cb=11)
00:00:02.949520
00:00:02.949521 [/Devices/piix3ide/0/LUN#999/Config/] (level 5) (restricted root)
00:00:02.949523   DeviceInstance       <string>  = "piix3ide/0" (cb=11)
00:00:02.949524   First                <integer> = 0x0000000000000000 (0)
00:00:02.949525   HasMediumAttachments <integer> = 0x0000000000000001 (1)
00:00:02.949526   Last                 <integer> = 0x0000000000000003 (3)
00:00:02.949528   iLedSet              <integer> = 0x0000000000000003 (3)
00:00:02.949529
00:00:02.949529 [/Devices/serial/] (level 2)
00:00:02.949531
00:00:02.949531 [/Devices/usb-ehci/] (level 2)
00:00:02.949533
00:00:02.949533 [/Devices/usb-ehci/0/] (level 3)
00:00:02.949535   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:02.949536   PCIDeviceNo   <integer> = 0x000000000000000b (11)
00:00:02.949537   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:02.949538   Trusted       <integer> = 0x0000000000000001 (1)
00:00:02.949539
00:00:02.949540 [/Devices/usb-ehci/0/Config/] (level 4) (restricted root)
00:00:02.949541
00:00:02.949542 [/Devices/usb-ehci/0/LUN#0/] (level 4)
00:00:02.949543   Driver <string>  = "VUSBRootHub" (cb=12)
00:00:02.949544
00:00:02.949545 [/Devices/usb-ehci/0/LUN#0/Config/] (level 5) (restricted root)
00:00:02.949547
00:00:02.949547 [/Devices/usb-ehci/0/LUN#999/] (level 4)
00:00:02.949549   Driver <string>  = "MainStatus" (cb=11)
00:00:02.949549
00:00:02.949550 [/Devices/usb-ehci/0/LUN#999/Config/] (level 5) (restricted root)
00:00:02.949552   First                <integer> = 0x0000000000000000 (0)
00:00:02.949553   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:02.949554   Last                 <integer> = 0x0000000000000000 (0)
00:00:02.949556   iLedSet              <integer> = 0x0000000000000002 (2)
00:00:02.949557
00:00:02.949557 [/Devices/usb-ohci/] (level 2)
00:00:02.949559
00:00:02.949559 [/Devices/usb-ohci/0/] (level 3)
00:00:02.949561   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:02.949562   PCIDeviceNo   <integer> = 0x0000000000000006 (6)
00:00:02.949563   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:02.949564   Trusted       <integer> = 0x0000000000000001 (1)
00:00:02.949565
00:00:02.949566 [/Devices/usb-ohci/0/Config/] (level 4) (restricted root)
00:00:02.949568
00:00:02.949568 [/Devices/usb-ohci/0/LUN#0/] (level 4)
00:00:02.949569   Driver <string>  = "VUSBRootHub" (cb=12)
00:00:02.949570
00:00:02.949571 [/Devices/usb-ohci/0/LUN#0/Config/] (level 5) (restricted root)
00:00:02.949573
00:00:02.949573 [/Devices/usb-ohci/0/LUN#999/] (level 4)
00:00:02.949575   Driver <string>  = "MainStatus" (cb=11)
00:00:02.949575
00:00:02.949577 [/Devices/usb-ohci/0/LUN#999/Config/] (level 5) (restricted root)
00:00:02.949579   First                <integer> = 0x0000000000000000 (0)
00:00:02.949581   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:02.949582   Last                 <integer> = 0x0000000000000000 (0)
00:00:02.949583   iLedSet              <integer> = 0x0000000000000001 (1)
00:00:02.949585
00:00:02.949585 [/Devices/vga/] (level 2)
00:00:02.949586
00:00:02.949587 [/Devices/vga/0/] (level 3)
00:00:02.949588   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:02.949590   PCIDeviceNo   <integer> = 0x0000000000000002 (2)
00:00:02.949591   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:02.949592   Trusted       <integer> = 0x0000000000000001 (1)
00:00:02.949593
00:00:02.949593 [/Devices/vga/0/Config/] (level 4) (restricted root)
00:00:02.949595   3DEnabled        <integer> = 0x0000000000000000 (0)
00:00:02.949597   CustomVideoModes <integer> = 0x0000000000000000 (0)
00:00:02.949598   FadeIn           <integer> = 0x0000000000000001 (1)
00:00:02.949599   FadeOut          <integer> = 0x0000000000000001 (1)
00:00:02.949600   HeightReduction  <integer> = 0x0000000000000000 (0)
00:00:02.949602   LogoFile         <string>  = "" (cb=1)
00:00:02.949603   LogoTime         <integer> = 0x0000000000000000 (0)
00:00:02.949604   MonitorCount     <integer> = 0x0000000000000008 (8)
00:00:02.949605   ShowBootMenu     <integer> = 0x0000000000000002 (2)
00:00:02.949606   VRamSize         <integer> = 0x0000000008000000 (134 217 728, 128.0 MiB)
00:00:02.949609
00:00:02.949609 [/Devices/vga/0/LUN#0/] (level 4)
00:00:02.949611   Driver <string>  = "MainDisplay" (cb=12)
00:00:02.949612
00:00:02.949612 [/Devices/vga/0/LUN#0/Config/] (level 5) (restricted root)
00:00:02.949614
00:00:02.949614 [/Devices/vga/0/LUN#999/] (level 4)
00:00:02.949616   Driver <string>  = "MainStatus" (cb=11)
00:00:02.949617
00:00:02.949617 [/Devices/vga/0/LUN#999/Config/] (level 5) (restricted root)
00:00:02.949619   First                <integer> = 0x0000000000000000 (0)
00:00:02.949621   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:02.949622   Last                 <integer> = 0x0000000000000000 (0)
00:00:02.949623   iLedSet              <integer> = 0x0000000000000000 (0)
00:00:02.949624
00:00:02.949625 [/Devices/virtio-net/] (level 2)
00:00:02.949626
00:00:02.949626 [/EM/] (level 1)
00:00:02.949628   TripleFaultReset <integer> = 0x0000000000000000 (0)
00:00:02.949629
00:00:02.949629 [/GCM/] (level 1)
00:00:02.949630
00:00:02.949630 [/GIM/] (level 1)
00:00:02.949632   Provider <string>  = "None" (cb=5)
00:00:02.949633
00:00:02.949633 [/HM/] (level 1)
00:00:02.949634   64bitEnabled        <integer> = 0x0000000000000000 (0)
00:00:02.949635   EnableLargePages    <integer> = 0x0000000000000000 (0)
00:00:02.949637   EnableNestedPaging  <integer> = 0x0000000000000001 (1)
00:00:02.949638   EnableUX            <integer> = 0x0000000000000001 (1)
00:00:02.949639   EnableVPID          <integer> = 0x0000000000000001 (1)
00:00:02.949640   Exclusive           <integer> = 0x0000000000000001 (1)
00:00:02.949642   HMForced            <integer> = 0x0000000000000001 (1)
00:00:02.949643   IBPBOnVMEntry       <integer> = 0x0000000000000000 (0)
00:00:02.949644   IBPBOnVMExit        <integer> = 0x0000000000000000 (0)
00:00:02.949645   L1DFlushOnSched     <integer> = 0x0000000000000001 (1)
00:00:02.949647   L1DFlushOnVMEntry   <integer> = 0x0000000000000000 (0)
00:00:02.949648   MDSClearOnSched     <integer> = 0x0000000000000001 (1)
00:00:02.949649   MDSClearOnVMEntry   <integer> = 0x0000000000000000 (0)
00:00:02.949650   SpecCtrlByHost      <integer> = 0x0000000000000000 (0)
00:00:02.949651   SvmVirtVmsaveVmload <integer> = 0x0000000000000000 (0)
00:00:02.949652   UseNEMInstead       <integer> = 0x0000000000000000 (0)
00:00:02.949654
00:00:02.949654 [/MM/] (level 1)
00:00:02.949655   CanUseLargerHeap <integer> = 0x0000000000000000 (0)
00:00:02.949656
00:00:02.949656 [/NEM/] (level 1)
00:00:02.949658   Allow64BitGuests  <integer> = 0x0000000000000000 (0)
00:00:02.949660   IBPBOnVMEntry     <integer> = 0x0000000000000000 (0)
00:00:02.949662   IBPBOnVMExit      <integer> = 0x0000000000000000 (0)
00:00:02.949663   L1DFlushOnSched   <integer> = 0x0000000000000001 (1)
00:00:02.949664   L1DFlushOnVMEntry <integer> = 0x0000000000000000 (0)
00:00:02.949665   MDSClearOnSched   <integer> = 0x0000000000000001 (1)
00:00:02.949666   MDSClearOnVMEntry <integer> = 0x0000000000000000 (0)
00:00:02.949667
00:00:02.949668 [/PDM/] (level 1)
00:00:02.949669
00:00:02.949669 [/PDM/AsyncCompletion/] (level 2)
00:00:02.949671
00:00:02.949671 [/PDM/AsyncCompletion/File/] (level 3)
00:00:02.949672
00:00:02.949673 [/PDM/AsyncCompletion/File/BwGroups/] (level 4)
00:00:02.949674
00:00:02.949675 [/PDM/BlkCache/] (level 2)
00:00:02.949676   CacheSize <integer> = 0x0000000000500000 (5 242 880, 5.0 MiB)
00:00:02.949678
00:00:02.949678 [/PDM/Devices/] (level 2)
00:00:02.949680
00:00:02.949680 [/PDM/Drivers/] (level 2)
00:00:02.949681
00:00:02.949682 [/PDM/Drivers/VBoxC/] (level 3)
00:00:02.949683   Path <string>  = "/usr/lib/virtualbox/components/VBoxC" (cb=37)
00:00:02.949684
00:00:02.949685 [/PDM/NetworkShaper/] (level 2)
00:00:02.949686
00:00:02.949686 [/PDM/NetworkShaper/BwGroups/] (level 3)
00:00:02.949688
00:00:02.949688 [/TM/] (level 1)
00:00:02.949689   UTCOffset <integer> = 0x0000000000000000 (0)
00:00:02.949690
00:00:02.949691 [/USB/] (level 1)
00:00:02.949692
00:00:02.949692 [/USB/USBProxy/] (level 2)
00:00:02.949694
00:00:02.949694 [/USB/USBProxy/GlobalConfig/] (level 3)
00:00:02.949695
00:00:02.949696 !!
00:00:02.949697 !! {clocks}
00:00:02.949697 !!
00:00:02.949699 Cpu Tick:         9454094372 (0x00000233820824) 3392292995Hz paused - virtualized - virtual clock
00:00:02.949703 Cpu Tick:         9456288351 (0x00000233a3825f) 3392292995Hz paused - virtualized - virtual clock
00:00:02.949706 Cpu Tick:         9456960252 (0x00000233adc2fc) 3392292995Hz paused - virtualized - virtual clock
00:00:02.949708 Cpu Tick:         9457778941 (0x00000233ba40fd) 3392292995Hz paused - virtualized - virtual clock
00:00:02.949711  Virtual:         2788069119 (0x000000a62e8eff) 1000000000Hz paused
00:00:02.949713 VirtSync:         2788019477 (0x000000a62dcd15) paused
00:00:02.949715           offset 47495  catch-up rate 10 %
00:00:02.949717     Real:            1901912 (0x000000001d0558) 1000Hz
00:00:02.949719 !!
00:00:02.949720 !! {cmos1}
00:00:02.949720 !!
00:00:02.949721 First CMOS bank, offsets 0x0E - 0x7F
00:00:02.949721 Offset 00 : --- use 'info rtc' to show CMOS clock --- 00 00
00:00:02.949723 Offset 10 : 00 00 f0 00 0e 80 02 ff-ff 2f 00 00 00 00 08 02
00:00:02.949727 Offset 20 : 80 ff ff ff ff 3f 00 00-00 00 00 00 00 00 08 72
00:00:02.949755 Offset 30 : ff ff 20 00 00 df 00 20-20 00 00 00 00 31 00 00
00:00:02.949760 Offset 40 : 00 00 00 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949764 Offset 50 : 00 00 00 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949768 Offset 60 : 04 00 20 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949773 Offset 70 : 00 00 00 00 00 00 00 00-01 00 00 00 00 00 00 00
00:00:02.949778 !!
00:00:02.949778 !! {cmos2}
00:00:02.949778 !!
00:00:02.949779 Second CMOS bank, offsets 0x80 - 0xFF
00:00:02.949779 Offset 80 : 00 00 18 00 ff ff ff ff-ff ff 00 00 00 00 00 00
00:00:02.949783 Offset 90 : 00 00 00 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949788 Offset a0 : 00 00 00 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949792 Offset b0 : 00 00 00 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949814 Offset c0 : 00 00 00 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949818 Offset d0 : 00 00 00 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949822 Offset e0 : 00 00 00 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949826 Offset f0 : 00 00 00 00 00 00 00 00-00 00 00 00 00 00 00 00
00:00:02.949831 !!
00:00:02.949832 !! {cpuidhost}
00:00:02.949832 !!
00:00:02.950042          Raw Standard CPUID Leaves
00:00:02.950042      Leaf/sub-leaf  eax      ebx      ecx      edx
00:00:02.950043 Hst: 00000000/0000  0000000d 756e6547 6c65746e 49656e69
00:00:02.950045 Hst: 00000001/0000  000306a9 06100800 7fbae3ff bfebfbff
00:00:02.950050 Hst: 00000002/0000  76035a01 00f0b2ff 00000000 00ca0000
00:00:02.950051 Hst: 00000003/0000  00000000 00000000 00000000 00000000
00:00:02.950053 Hst: 00000004/0000  1c004121 01c0003f 0000003f 00000000
00:00:02.950054 Hst: 00000004/0001  1c004122 01c0003f 0000003f 00000000
00:00:02.950056 Hst: 00000004/0002  1c004143 01c0003f 000001ff 00000000
00:00:02.950057 Hst: 00000004/0003  1c03c163 03c0003f 00001fff 00000006
00:00:02.950059 Hst: 00000004/0004  00000000 00000000 00000000 00000000
00:00:02.950060 Hst: 00000005/0000  00000040 00000040 00000003 00001120
00:00:02.950062 Hst: 00000006/0000  00000077 00000002 00000009 00000000
00:00:02.950063 Hst: 00000007/0000  00000000 00000281 00000000 9c000400
00:00:02.950065 Hst: 00000007/0001  00000000 00000000 00000000 00000000
00:00:02.950066 Hst: 00000007/0002  00000000 00000000 00000000 00000000
00:00:02.950067 Hst: 00000008/0000  00000000 00000000 00000000 00000000
00:00:02.950069 Hst: 00000009/0000  00000000 00000000 00000000 00000000
00:00:02.950070 Hst: 0000000a/0000  07300403 00000000 00000000 00000603
00:00:02.950072 Hst: 0000000b/0000  00000001 00000002 00000100 00000006
00:00:02.950073 Hst: 0000000b/0001  00000004 00000008 00000201 00000006
00:00:02.950074 Hst: 0000000b/0002  00000000 00000000 00000002 00000006
00:00:02.950084 Hst: 0000000c/0000  00000000 00000000 00000000 00000000
00:00:02.950085 Hst: 0000000d/0000  00000007 00000340 00000340 00000000
00:00:02.950087 Hst: 0000000d/0001  00000001 00000000 00000000 00000000
00:00:02.950088 Hst: 0000000d/0002  00000100 00000240 00000000 00000000
00:00:02.950089 Hst: 0000000d/0003  00000000 00000000 00000000 00000000
00:00:02.950091                                Name: GenuineIntel
00:00:02.950092                            Supports: 0x00000000-0x0000000d
00:00:02.950094                              Family:  6 	Extended: 0 	Effective: 6
00:00:02.950095                               Model: 10 	Extended: 3 	Effective: 58
00:00:02.950097                            Stepping: 9
00:00:02.950098                                Type: 0 (primary)
00:00:02.950099                             APIC ID: 0x06
00:00:02.950100                        Logical CPUs: 16
00:00:02.950101                        CLFLUSH Size: 8
00:00:02.950101                            Brand ID: 0x00
00:00:02.950106                         Features EDX FPU VME DE PSE TSC MSR PAE MCE CX8 APIC SEP MTRR PGE MCA CMOV PAT PSE-36 CLFSH DS ACPI MMX FXSR SSE SSE2 SS HTT TM PBE
00:00:02.950113                         Features ECX SSE3 PCLMUL DTES64 MONITOR CPL-DS VMX SMX EST TM2 SSSE3 CX16 TPRUPDATE PDCM PCID SSE4_1 SSE4_2 X2APIC POPCNT TSCDEADL AES XSAVE OSXSAVE AVX F16C RDRAND
00:00:02.950120 Structured Extended Feature Flags Enumeration (leaf 7):
00:00:02.950121                  Ext Features #0 EBX FSGSBASE SMEP ERMS
00:00:02.950123                  Ext Features #0 ECX
00:00:02.950124                  Ext Features #0 EDX MD_CLEAR IBRS_IBPB STIBP FLUSH_CMD SSBD
00:00:02.950126 Processor Extended State Enumeration (leaf 0xd):
00:00:02.950127    XSAVE area cur/max size by XCR0, Host: 0x340/0x340
00:00:02.950128                    Valid XCR0 bits, Host: 0x00000000`00000007 ( x87 SSE YMM_Hi128 )
00:00:02.950131                     XSAVE features, Host XSAVEOPT
00:00:02.950132       XSAVE area cur size XCR0|XSS, Host: 0x0
00:00:02.950133                Valid IA32_XSS bits, Host: 0x00000000`00000000
00:00:02.950135   State #2, Host: off=0x0240, cb=0x0100 IA32_XSS-bit -- YMM_Hi128
00:00:02.950137          Raw Extended CPUID Leaves
00:00:02.950138      Leaf/sub-leaf  eax      ebx      ecx      edx
00:00:02.950138 Hst: 80000000/0000  80000008 00000000 00000000 00000000
00:00:02.950140 Hst: 80000001/0000  00000000 00000000 00000001 28100800
00:00:02.950142 Hst: 80000002/0000  20202020 20202020 65746e49 2952286c
00:00:02.950144 Hst: 80000003/0000  726f4320 4d542865 37692029 3737332d
00:00:02.950146 Hst: 80000004/0000  50432030 20402055 30342e33 007a4847
00:00:02.950147 Hst: 80000005/0000  00000000 00000000 00000000 00000000
00:00:02.950151 Hst: 80000006/0000  00000000 00000000 01006040 00000000
00:00:02.950153 Hst: 80000007/0000  00000000 00000000 00000000 00000100
00:00:02.950154 Hst: 80000008/0000  00003024 00000000 00000000 00000000
00:00:02.950156 Ext Name:
00:00:02.950157 Ext Supports:                    0x80000000-0x80000008
00:00:02.950158 Family:                          0  	Extended: 0 	Effective: 0
00:00:02.950158 Model:                           0  	Extended: 0 	Effective: 0
00:00:02.950159 Stepping:                        0
00:00:02.950159 Brand ID:                        0x000
00:00:02.950161                   Ext Features EDX SEP NX RDTSCP LM
00:00:02.950163                   Ext Features ECX FPU
00:00:02.950164 Full Name:                       "        Intel(R) Core(TM) i7-3770 CPU @ 3.40GHz"
00:00:02.950165 TLB 2/4M Instr/Uni:              res0     0 entries
00:00:02.950166 TLB 2/4M Data:                   res0     0 entries
00:00:02.950166 TLB 4K Instr/Uni:                res0     0 entries
00:00:02.950167 TLB 4K Data:                     res0     0 entries
00:00:02.950168 L1 Instr Cache Line Size:        0 bytes
00:00:02.950168 L1 Instr Cache Lines Per Tag:    0
00:00:02.950169 L1 Instr Cache Associativity:    res0
00:00:02.950169 L1 Instr Cache Size:             0 KB
00:00:02.950170 L1 Data Cache Line Size:         0 bytes
00:00:02.950170 L1 Data Cache Lines Per Tag:     0
00:00:02.950171 L1 Data Cache Associativity:     res0
00:00:02.950171 L1 Data Cache Size:              0 KB
00:00:02.950172 L2 TLB 2/4M Instr/Uni:           off       0 entries
00:00:02.950172 L2 TLB 2/4M Data:                off       0 entries
00:00:02.950173 L2 TLB 4K Instr/Uni:             off       0 entries
00:00:02.950174 L2 TLB 4K Data:                  off       0 entries
00:00:02.950175 L2 Cache Line Size:              64 bytes
00:00:02.950175 L2 Cache Lines Per Tag:          0
00:00:02.950175 L2 Cache Associativity:          8 way
00:00:02.950176 L2 Cache Size:                   256 KB
00:00:02.950177 L3 Cache Line Size:              0 bytes
00:00:02.950177 L3 Cache Lines Per Tag:          0
00:00:02.950177 L3 Cache Associativity:          off
00:00:02.950178 L3 Cache Size:                   0 KB
00:00:02.950179 APM Features EDX
00:00:02.950179   Mnemonic - Description                                  = Host
00:00:02.950181   TS - Temperature Sensor                                 = 0
00:00:02.950183   FID - Frequency ID control                              = 0
00:00:02.950185   VID - Voltage ID control                                = 0
00:00:02.950186   TTP - Thermal Trip                                      = 0
00:00:02.950188   TM - Hardware Thermal Control (HTC)                     = 0
00:00:02.950189   100MHzSteps - 100 MHz Multiplier control                = 0
00:00:02.950191   HwPstate - Hardware P-state control                     = 0
00:00:02.950192   TscInvariant - Invariant Time Stamp Counter             = 1
00:00:02.950200   CPB - Core Performance Boost                            = 0
00:00:02.950202   EffFreqRO - Read-only Effective Frequency Interface     = 0
00:00:02.950203   ProcFdbkIf - Processor Feedback Interface               = 0
00:00:02.950204   ProcPwrRep - Core power reporting interface support     = 0
00:00:02.950205   ConnectedStandby - Connected Standby                    = 0
00:00:02.950207   RAPL - Running average power limit                      = 0
00:00:02.950208 Physical Address Width:          36 bits
00:00:02.950209 Virtual Address Width:           48 bits
00:00:02.950210 Max page count for INVLPGB:      0x3024
00:00:02.950210 Max ECX for RDPRU:               0x0
00:00:02.950212 !!
00:00:02.950212 !! {cpuload}
00:00:02.950212 !!
00:00:02.950214     CPU load for virtual CPU 0x00
00:00:02.950214     -------------------------------
00:00:02.950215   1s: OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO
00:00:02.950216   0s: OOOOOOOOOOOOOOOOOOOOOOOOOOOOOO
00:00:02.950217     (#=guest, O=VMM overhead)  idCpu=0x0
00:00:02.950218
00:00:02.950218     CPU load for virtual CPU 0x01
00:00:02.950220     -------------------------------
00:00:02.950221   1s: OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO
00:00:02.950222   0s: O
00:00:02.950223     (#=guest, O=VMM overhead)  idCpu=0x1
00:00:02.950223
00:00:02.950224     CPU load for virtual CPU 0x02
00:00:02.950224     -------------------------------
00:00:02.950225   1s: OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO
00:00:02.950225   0s: O
00:00:02.950226     (#=guest, O=VMM overhead)  idCpu=0x2
00:00:02.950226
00:00:02.950227     CPU load for virtual CPU 0x03
00:00:02.950227     -------------------------------
00:00:02.950228   1s: OOOOOOOOOOOOOOO
00:00:02.950228   0s: OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO
00:00:02.950229     (#=guest, O=VMM overhead)  idCpu=0x3
00:00:02.950231 !!
00:00:02.950231 !! {cpumvmxfeat}
00:00:02.950232 !!
00:00:02.950233 Nested hardware virtualization - VMX features
00:00:02.950233   Mnemonic - Description                                  = guest (host)
00:00:02.950233   VMX - Virtual-Machine Extensions                        = 0 (1)
00:00:02.950234   InsOutInfo - INS/OUTS instruction info.                 = 0 (1)
00:00:02.950235   ExtIntExit - External interrupt exiting                 = 0 (1)
00:00:02.950236   NmiExit - NMI exiting                                   = 0 (1)
00:00:02.950237   VirtNmi - Virtual NMIs                                  = 0 (1)
00:00:02.950238   PreemptTimer - VMX preemption timer                     = 0 (1)
00:00:02.950239   PostedInt - Posted interrupts                           = 0 (0)
00:00:02.950239   IntWindowExit - Interrupt-window exiting                = 0 (1)
00:00:02.950240   TscOffsetting - TSC offsetting                          = 0 (1)
00:00:02.950241   HltExit - HLT exiting                                   = 0 (1)
00:00:02.950242   InvlpgExit - INVLPG exiting                             = 0 (1)
00:00:02.950243   MwaitExit - MWAIT exiting                               = 0 (1)
00:00:02.950243   RdpmcExit - RDPMC exiting                               = 0 (1)
00:00:02.950244   RdtscExit - RDTSC exiting                               = 0 (1)
00:00:02.950245   Cr3LoadExit - CR3-load exiting                          = 0 (1)
00:00:02.950246   Cr3StoreExit - CR3-store exiting                        = 0 (1)
00:00:02.950246   TertiaryExecCtls - Activate tertiary controls           = 0 (0)
00:00:02.950247   Cr8LoadExit  - CR8-load exiting                         = 0 (1)
00:00:02.950248   Cr8StoreExit - CR8-store exiting                        = 0 (1)
00:00:02.950249   UseTprShadow - Use TPR shadow                           = 0 (1)
00:00:02.950250   NmiWindowExit - NMI-window exiting                      = 0 (1)
00:00:02.950250   MovDRxExit - Mov-DR exiting                             = 0 (1)
00:00:02.950251   UncondIoExit - Unconditional I/O exiting                = 0 (1)
00:00:02.950252   UseIoBitmaps - Use I/O bitmaps                          = 0 (1)
00:00:02.950253   MonitorTrapFlag - Monitor Trap Flag                     = 0 (1)
00:00:02.950254   UseMsrBitmaps - MSR bitmaps                             = 0 (1)
00:00:02.950254   MonitorExit - MONITOR exiting                           = 0 (1)
00:00:02.950255   PauseExit - PAUSE exiting                               = 0 (1)
00:00:02.950256   SecondaryExecCtl - Activate secondary controls          = 0 (1)
00:00:02.950257   VirtApic - Virtualize-APIC accesses                     = 0 (1)
00:00:02.950258   Ept - Extended Page Tables                              = 0 (1)
00:00:02.950258   DescTableExit - Descriptor-table exiting                = 0 (1)
00:00:02.950259   Rdtscp - Enable RDTSCP                                  = 0 (1)
00:00:02.950260   VirtX2ApicMode - Virtualize-x2APIC mode                 = 0 (1)
00:00:02.950261   Vpid - Enable VPID                                      = 0 (1)
00:00:02.950262   WbinvdExit - WBINVD exiting                             = 0 (1)
00:00:02.950263   UnrestrictedGuest - Unrestricted guest                  = 0 (1)
00:00:02.950263   ApicRegVirt - APIC-register virtualization              = 0 (0)
00:00:02.950264   VirtIntDelivery - Virtual-interrupt delivery            = 0 (0)
00:00:02.950265   PauseLoopExit - PAUSE-loop exiting                      = 0 (0)
00:00:02.950266   RdrandExit - RDRAND exiting                             = 0 (1)
00:00:02.950267   Invpcid - Enable INVPCID                                = 0 (0)
00:00:02.950267   VmFuncs - Enable VM Functions                           = 0 (0)
00:00:02.950268   VmcsShadowing - VMCS shadowing                          = 0 (0)
00:00:02.950269   RdseedExiting - RDSEED exiting                          = 0 (0)
00:00:02.950270   PML - Page-Modification Log                             = 0 (0)
00:00:02.950270   EptVe - EPT violations can cause #VE                    = 0 (0)
00:00:02.950271   ConcealVmxFromPt - Conceal VMX from Processor Trace     = 0 (0)
00:00:02.950272   XsavesXRstors - Enable XSAVES/XRSTORS                   = 0 (0)
00:00:02.950273   PasidTranslate - PASID translation                      = 0 (0)
00:00:02.950274   ModeBasedExecuteEpt - Mode-based execute permissions    = 0 (0)
00:00:02.950274   SppEpt - Sub-page page write permissions for EPT        = 0 (0)
00:00:02.950275   PtEpt - Processor Trace address' translatable by EPT    = 0 (0)
00:00:02.950276   UseTscScaling - Use TSC scaling                         = 0 (0)
00:00:02.950277   UserWaitPause - Enable TPAUSE, UMONITOR and UMWAIT      = 0 (0)
00:00:02.950278   Pconfig - Enable PCONFIG                                = 0 (0)
00:00:02.950278   EnclvExit - ENCLV exiting                               = 0 (0)
00:00:02.950279   BusLockDetect - VMM Bus-Lock detection                  = 0 (0)
00:00:02.950280   InstrTimeout - Instruction timeout                      = 0 (0)
00:00:02.950281   LoadIwKeyExit - LOADIWKEY exiting                       = 0 (0)
00:00:02.950282   HLAT - Hypervisor-managed linear-address translation    = 0 (0)
00:00:02.950282   EptPagingWrite - EPT paging-write                       = 0 (0)
00:00:02.950283   GstPagingVerify - Guest-paging verification             = 0 (0)
00:00:02.950284   IpiVirt - IPI virtualization                            = 0 (0)
00:00:02.950285   VirtSpecCtrl - Virtualize IA32_SPEC_CTRL                = 0 (0)
00:00:02.950286   EntryLoadDebugCtls - Load debug controls on VM-entry    = 0 (1)
00:00:02.950286   Ia32eModeGuest - IA-32e mode guest                      = 0 (1)
00:00:02.950287   EntryLoadEferMsr - Load IA32_EFER MSR on VM-entry       = 0 (1)
00:00:02.950288   EntryLoadPatMsr - Load IA32_PAT MSR on VM-entry         = 0 (1)
00:00:02.950289   ExitSaveDebugCtls - Save debug controls on VM-exit      = 0 (1)
00:00:02.950290   HostAddrSpaceSize - Host address-space size             = 0 (1)
00:00:02.950359   ExitAckExtInt - Acknowledge interrupt on VM-exit        = 0 (1)
00:00:02.950361   ExitSavePatMsr - Save IA32_PAT MSR on VM-exit           = 0 (1)
00:00:02.950362   ExitLoadPatMsr - Load IA32_PAT MSR on VM-exit           = 0 (1)
00:00:02.950362   ExitSaveEferMsr - Save IA32_EFER MSR on VM-exit         = 0 (1)
00:00:02.950363   ExitLoadEferMsr - Load IA32_EFER MSR on VM-exit         = 0 (1)
00:00:02.950364   SavePreemptTimer - Save VMX-preemption timer            = 0 (1)
00:00:02.950365   SecondaryExitCtls - Secondary VM-exit controls          = 0 (0)
00:00:02.950366   ExitSaveEferLma - Save IA32_EFER.LMA on VM-exit         = 0 (1)
00:00:02.950366   IntelPt - Intel Processor Trace in VMX operation        = 0 (0)
00:00:02.950367   VmwriteAll - VMWRITE to any supported VMCS field        = 0 (0)
00:00:02.950368   EntryInjectSoftInt - Inject softint. with 0-len instr.  = 0 (0)
00:00:02.950369 !!
00:00:02.950369 !! {critsect}
00:00:02.950370 !!
00:00:02.950371 00007fd550534300: 'acpi#0'
00:00:02.950373 00007fd550abd5d0: 'OHCI#0Irq'
00:00:02.950374 00007fd550abd700: 'usb-ohci#0Auto' default used-by-timer-or-similar
00:00:02.950376 00007fd550ac0490: 'EHCI#0Irq'
00:00:02.950377 00007fd550ac05c0: 'usb-ehci#0Auto' default used-by-timer-or-similar
00:00:02.950379 00007fd550ac2880: 'AC'97'
00:00:02.950380 00007fd550ac4520: 'PCnet#0' used-by-timer-or-similar
00:00:02.950382 00007fd52ae055e8: 'ATA#1-Req'
00:00:02.950383 00007fd52ae05488: 'ATA#1-Ctl'
00:00:02.950384 00007fd52ad025e8: 'ATA#0-Req'
00:00:02.950386 00007fd52ad02488: 'ATA#0-Ctl'
00:00:02.950387 00007fd5505522a0: 'VGA#0_IRQ'
00:00:02.950388 00007fd550552130: 'VGA#0' used-by-timer-or-similar
00:00:02.950390 00007fd550fdf700: 'VMMDev#0'
00:00:02.950391 00007fd550fe4440: '8237A#0Auto' default used-by-timer-or-similar
00:00:02.950392 00007fd550fe5480: 'mc146818#0Auto' default used-by-timer-or-similar
00:00:02.950394 00007fd550fe6380: 'pit#0'
00:00:02.950395 00007fd550ff7600: 'pckbd#0Auto' default used-by-timer-or-similar
00:00:02.950396 00007fd54340b380: 'pcbios#0Auto' default used-by-timer-or-similar
00:00:02.950398 00007fd5434091c0: 'pcarch#0Auto' default
00:00:02.950399 00007fd5430ef340: 'NOP' used-by-timer-or-similar nop
00:00:02.950401 00007fd5430ef240: 'PDM'
00:00:02.950402 00007fd5430f6ac0: 'TM tsc queue timer lock'
00:00:02.950403 00007fd5430f6840: 'TM virtual_sync queue timer lock'
00:00:02.950405 00007fd5430f65c0: 'TM virtual queue timer lock'
00:00:02.950406 00007fd5430f6340: 'TM real queue timer lock'
00:00:02.950407 00007fd5430f6e80: 'TM VirtualSync Lock'
00:00:02.950408 00007fd5430e2040: 'PGM'
00:00:02.950410 !!
00:00:02.950410 !! {critsectrw}
00:00:02.950410 !!
00:00:02.950411 00007fd5430f4b40: 'IOM Lock'
00:00:02.950419 00007fd5430f6bc0: 'TM tsc queue alloc lock'
00:00:02.950420 00007fd5430f6940: 'TM virtual_sync queue alloc lock'
00:00:02.950421 00007fd5430f66c0: 'TM virtual queue alloc lock'
00:00:02.950422 00007fd5430f6440: 'TM real queue alloc lock'
00:00:02.950424 !!
00:00:02.950424 !! {dmac}
00:00:02.950424 !!
00:00:02.950425
00:00:02.950425 DMAC0:
00:00:02.950426  Status : 00 - DRQ 3210  TC 3210
00:00:02.950427                    0000     0000
00:00:02.950428  Mask   : FB - Chn 3210
00:00:02.950429                    1011
00:00:02.950430  Temp   : 00
00:00:02.950431  Command: 00
00:00:02.950431   DACK: active low          DREQ: active low
00:00:02.950432   Extended write: disabled  Priority: fixed
00:00:02.950433   Timing: compressed        Controller: disabled
00:00:02.950434   Adress Hold: disabled     Mem-to-Mem Ch 0/1: disabled
00:00:02.950434
00:00:02.950435  DMA Channel 0:  Page:00
00:00:02.950436   Mode : 00   Auto-init: no  Increment
00:00:02.950437     Xfer Type: verify   Mode: demand
00:00:02.950437   Base    address:0000  count:0000
00:00:02.950438   Current address:0000  count:0000
00:00:02.950439
00:00:02.950439  DMA Channel 1:  Page:00
00:00:02.950440   Mode : 00   Auto-init: no  Increment
00:00:02.950441     Xfer Type: verify   Mode: demand
00:00:02.950441   Base    address:0000  count:0000
00:00:02.950442   Current address:0000  count:0000
00:00:02.950443
00:00:02.950443  DMA Channel 2:  Page:00
00:00:02.950443   Mode : 00   Auto-init: no  Increment
00:00:02.950444     Xfer Type: verify   Mode: demand
00:00:02.950445   Base    address:0000  count:0000
00:00:02.950445   Current address:0000  count:0000
00:00:02.950446
00:00:02.950446  DMA Channel 3:  Page:00
00:00:02.950447   Mode : 00   Auto-init: no  Increment
00:00:02.950448     Xfer Type: verify   Mode: demand
00:00:02.950448   Base    address:0000  count:0000
00:00:02.950449   Current address:0000  count:0000
00:00:02.950450
00:00:02.950450 DMAC1:
00:00:02.950450  Status : 00 - DRQ 3210  TC 3210
00:00:02.950451                    0000     0000
00:00:02.950452  Mask   : FE - Chn 3210
00:00:02.950453                    1110
00:00:02.950454  Temp   : 00
00:00:02.950454  Command: 00
00:00:02.950455   DACK: active low          DREQ: active low
00:00:02.950456   Extended write: disabled  Priority: fixed
00:00:02.950456   Timing: compressed        Controller: disabled
00:00:02.950457   Adress Hold: disabled     Mem-to-Mem Ch 0/1: disabled
00:00:02.950458
00:00:02.950458  DMA Channel 0:  Page:00
00:00:02.950459   Mode : C0   Auto-init: no  Increment
00:00:02.950459     Xfer Type: verify   Mode: cascade
00:00:02.950460   Base    address:0000  count:0000
00:00:02.950461   Current address:0000  count:0000
00:00:02.950461
00:00:02.950462  DMA Channel 1:  Page:00
00:00:02.950462   Mode : 00   Auto-init: no  Increment
00:00:02.950463     Xfer Type: verify   Mode: demand
00:00:02.950464   Base    address:0000  count:0000
00:00:02.950464   Current address:0000  count:0000
00:00:02.950465
00:00:02.950465  DMA Channel 2:  Page:00
00:00:02.950466   Mode : 00   Auto-init: no  Increment
00:00:02.950467     Xfer Type: verify   Mode: demand
00:00:02.950467   Base    address:0000  count:0000
00:00:02.950468   Current address:0000  count:0000
00:00:02.950469
00:00:02.950469  DMA Channel 3:  Page:00
00:00:02.950469   Mode : 00   Auto-init: no  Increment
00:00:02.950470     Xfer Type: verify   Mode: demand
00:00:02.950471   Base    address:0000  count:0000
00:00:02.950471   Current address:0000  count:0000
00:00:02.950472 !!
00:00:02.950473 !! {dmapage}
00:00:02.950473 !!
00:00:02.950474 DMA page registers at 80: 00 00 00 00 00 00 00 00
00:00:02.950477 DMA page registers at 88: 00 00 00 00 00 00 00 00
00:00:02.950479 !!
00:00:02.950480 !! {dtlb}
00:00:02.950480 !!
00:00:02.950483 000: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950487 001: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950490 002: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950493 003: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950495 004: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950497 005: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950500 006: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950502 007: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950504 008: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950507 009: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950509 00a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950511 00b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950514 00c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950516 00d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950518 00e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950521 00f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950523 010: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950525 011: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950528 012: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950530 013: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950532 014: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950535 015: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950537 016: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950540 017: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950542 018: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950544 019: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950547 01a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950549 01b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950551 01c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950554 01d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950556 01e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950558 01f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950561 020: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950563 021: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950565 022: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950568 023: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950570 024: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950572 025: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950575 026: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950577 027: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950579 028: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950582 029: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950584 02a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950586 02b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950589 02c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950591 02d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950593 02e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950596 02f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950598 030: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950600 031: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950603 032: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950605 033: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950607 034: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950610 035: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950612 036: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950614 037: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950617 038: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950619 039: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950621 03a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950624 03b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950626 03c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950628 03d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950630 03e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950633 03f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950635 040: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950637 041: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950640 042: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950642 043: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950645 044: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950647 045: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950649 046: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950652 047: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950654 048: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950656 049: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950659 04a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950661 04b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950663 04c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950666 04d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950668 04e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950670 04f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950673 050: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950675 051: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950677 052: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950679 053: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950682 054: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950684 055: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950687 056: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950689 057: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950691 058: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950694 059: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950696 05a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950698 05b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950701 05c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950703 05d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950705 05e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950708 05f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950710 060: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950713 061: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950715 062: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950717 063: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950720 064: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950722 065: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950724 066: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950726 067: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950729 068: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950731 069: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950733 06a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950736 06b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950738 06c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950740 06d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950743 06e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950745 06f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950747 070: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950750 071: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950752 072: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950754 073: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950757 074: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950759 075: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950762 076: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950764 077: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950766 078: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950769 079: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950771 07a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950773 07b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950776 07c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950778 07d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950780 07e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950783 07f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950785 080: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950787 081: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950790 082: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950792 083: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950794 084: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950797 085: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950799 086: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950801 087: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950804 088: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950806 089: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950808 08a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950811 08b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950813 08c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950815 08d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950818 08e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950820 08f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950823 090: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950825 091: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950827 092: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950830 093: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950832 094: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950834 095: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950837 096: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950839 097: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950841 098: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950844 099: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950846 09a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950848 09b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950851 09c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950853 09d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950855 09e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950858 09f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950860 0a0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950862 0a1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950865 0a2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950867 0a3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950869 0a4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950872 0a5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950874 0a6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950877 0a7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950879 0a8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950881 0a9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950884 0aa: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950886 0ab: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950888 0ac: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950891 0ad: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950893 0ae: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950895 0af: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950897 0b0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950900 0b1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950902 0b2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950905 0b3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950907 0b4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950909 0b5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950911 0b6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950914 0b7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950916 0b8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950918 0b9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950921 0ba: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950923 0bb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950925 0bc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950928 0bd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950930 0be: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950932 0bf: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950935 0c0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950937 0c1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950940 0c2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950942 0c3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950944 0c4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950947 0c5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950949 0c6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950951 0c7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950954 0c8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950956 0c9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950958 0ca: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950961 0cb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950963 0cc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950966 0cd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950968 0ce: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950970 0cf: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950973 0d0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950975 0d1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950977 0d2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950979 0d3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950982 0d4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950984 0d5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950986 0d6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950989 0d7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950991 0d8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950993 0d9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.950996 0da: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.950998 0db: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951000 0dc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951003 0dd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951005 0de: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951007 0df: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951010 0e0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951012 0e1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951014 0e2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951017 0e3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951019 0e4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951021 0e5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951023 0e6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951026 0e7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951028 0e8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951030 0e9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951033 0ea: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951035 0eb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951037 0ec: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951040 0ed: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951042 0ee: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951044 0ef: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951047 0f0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951049 0f1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951051 0f2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951054 0f3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951056 0f4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951058 0f5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951060 0f6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951063 0f7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951065 0f8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951068 0f9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951070 0fa: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951072 0fb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951074 0fc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951077 0fd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951079 0fe: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951082 0ff: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951084 100: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951086 101: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951089 102: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951091 103: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951093 104: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951096 105: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951098 106: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951101 107: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951103 108: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951106 109: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951109 10a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951111 10b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951114 10c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951116 10d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951118 10e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951121 10f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951123 110: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951126 111: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951129 112: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951131 113: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951133 114: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951136 115: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951139 116: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951141 117: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951144 118: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951146 119: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951148 11a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951151 11b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951154 11c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951156 11d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951159 11e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951162 11f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951164 120: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951166 121: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951169 122: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951171 123: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951174 124: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951176 125: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951179 126: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951181 127: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951184 128: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951186 129: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951188 12a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951191 12b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951193 12c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951196 12d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951198 12e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951201 12f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951203 130: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951206 131: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951209 132: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951211 133: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951213 134: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951216 135: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951218 136: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951221 137: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951223 138: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951226 139: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951228 13a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951231 13b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951233 13c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951235 13d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951238 13e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951240 13f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951242 140: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951245 141: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951247 142: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951249 143: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951252 144: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951254 145: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951256 146: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951259 147: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951261 148: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951263 149: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951266 14a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951268 14b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951270 14c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951273 14d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951275 14e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951277 14f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951280 150: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951282 151: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951284 152: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951287 153: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951289 154: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951291 155: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951294 156: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951296 157: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951298 158: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951301 159: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951303 15a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951305 15b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951308 15c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951310 15d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951312 15e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951315 15f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951317 160: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951319 161: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951322 162: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951324 163: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951326 164: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951329 165: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951331 166: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951333 167: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951336 168: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951338 169: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951340 16a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951343 16b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951345 16c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951347 16d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951350 16e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951352 16f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951354 170: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951357 171: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951359 172: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951361 173: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951364 174: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951366 175: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951368 176: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951371 177: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951373 178: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951375 179: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951378 17a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951380 17b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951382 17c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951385 17d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951387 17e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951389 17f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951392 180: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951394 181: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951396 182: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951399 183: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951401 184: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951404 185: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951406 186: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951408 187: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951411 188: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951418 189: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951420 18a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951423 18b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951425 18c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951428 18d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951430 18e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951432 18f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951435 190: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951437 191: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951439 192: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951442 193: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951444 194: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951447 195: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951449 196: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951452 197: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951454 198: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951456 199: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951459 19a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951461 19b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951463 19c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951466 19d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951468 19e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951471 19f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951473 1a0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951475 1a1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951478 1a2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951480 1a3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951482 1a4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951485 1a5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951487 1a6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951489 1a7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951492 1a8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951494 1a9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951496 1aa: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951499 1ab: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951506 1ac: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951508 1ad: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951511 1ae: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951513 1af: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951515 1b0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951518 1b1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951520 1b2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951523 1b3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951525 1b4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951527 1b5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951529 1b6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951532 1b7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951534 1b8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951536 1b9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951539 1ba: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951541 1bb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951543 1bc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951546 1bd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951548 1be: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951551 1bf: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951553 1c0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951555 1c1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951558 1c2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951560 1c3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951563 1c4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951565 1c5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951568 1c6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951570 1c7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951572 1c8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951575 1c9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951577 1ca: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951579 1cb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951582 1cc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951584 1cd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951586 1ce: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951589 1cf: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951591 1d0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951593 1d1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951596 1d2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951598 1d3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951600 1d4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951603 1d5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951605 1d6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951608 1d7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951610 1d8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951612 1d9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951615 1da: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951617 1db: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951619 1dc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951622 1dd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951624 1de: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951626 1df: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951629 1e0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951631 1e1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951633 1e2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951636 1e3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951638 1e4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951640 1e5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951642 1e6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951645 1e7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951647 1e8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951650 1e9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951652 1ea: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951654 1eb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951657 1ec: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951659 1ed: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951661 1ee: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951664 1ef: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951666 1f0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951669 1f1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951671 1f2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951673 1f3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951675 1f4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951678 1f5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951680 1f6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951682 1f7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951685 1f8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951687 1f9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951690 1fa: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951692 1fb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951694 1fc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951697 1fd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951699 1fe: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.951702 1ff: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.951706 !!
00:00:02.951706 !! {ehci}
00:00:02.951706 !!
00:00:02.951707 USBCMD: 80b00
00:00:02.951708     CMD_ASYNC_SCHED_PARK_ENABLE
00:00:02.951708     CMD_FRAME_LIST_SIZE              0
00:00:02.951709     CMD_ASYNC_SCHED_PARK_MODE_COUNT  3
00:00:02.951710     CMD_INTERRUPT_THRESHOLD          8
00:00:02.951710 USBSTS: 1000
00:00:02.951711     STATUS_HCHALTED
00:00:02.951712 USBINTR: 0
00:00:02.951712 FRINDEX: 0
00:00:02.951713 CTRLDSSEGMENT:    0
00:00:02.951714 PERIODICLISTBASE: 0
00:00:02.951714 ASYNCLISTADDR:    0
00:00:02.951715
00:00:02.951716 PORTSC for port 0:
00:00:02.951716     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951717     PORT_POWER
00:00:02.951718     PORT_OWNER (1 = owned by companion HC)
00:00:02.951718 PORTSC for port 1:
00:00:02.951719     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951719     PORT_POWER
00:00:02.951720     PORT_OWNER (1 = owned by companion HC)
00:00:02.951720 PORTSC for port 2:
00:00:02.951720     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951721     PORT_POWER
00:00:02.951721     PORT_OWNER (1 = owned by companion HC)
00:00:02.951722 PORTSC for port 3:
00:00:02.951722     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951723     PORT_POWER
00:00:02.951723     PORT_OWNER (1 = owned by companion HC)
00:00:02.951724 PORTSC for port 4:
00:00:02.951724     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951725     PORT_POWER
00:00:02.951725     PORT_OWNER (1 = owned by companion HC)
00:00:02.951726 PORTSC for port 5:
00:00:02.951726     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951727     PORT_POWER
00:00:02.951727     PORT_OWNER (1 = owned by companion HC)
00:00:02.951728 PORTSC for port 6:
00:00:02.951728     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951729     PORT_POWER
00:00:02.951729     PORT_OWNER (1 = owned by companion HC)
00:00:02.951730 PORTSC for port 7:
00:00:02.951730     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951731     PORT_POWER
00:00:02.951731     PORT_OWNER (1 = owned by companion HC)
00:00:02.951731 PORTSC for port 8:
00:00:02.951732     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951732     PORT_POWER
00:00:02.951733     PORT_OWNER (1 = owned by companion HC)
00:00:02.951733 PORTSC for port 9:
00:00:02.951734     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951734     PORT_POWER
00:00:02.951735     PORT_OWNER (1 = owned by companion HC)
00:00:02.951735 PORTSC for port 10:
00:00:02.951736     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951736     PORT_POWER
00:00:02.951737     PORT_OWNER (1 = owned by companion HC)
00:00:02.951737 PORTSC for port 11:
00:00:02.951738     LINE_STATUS:     SE0 (0), not low-speed
00:00:02.951738     PORT_POWER
00:00:02.951739     PORT_OWNER (1 = owned by companion HC)
00:00:02.951740 !!
00:00:02.951740 !! {exits}
00:00:02.951740 !!
00:00:02.951742 CPU[0]: VM-exit history:
00:00:02.951743    Exit No.:     TSC timestamp / delta    RIP (Flat/*)      Exit   Name
00:00:02.951745       76055: 0x000006082d7dc29a/+0        00000000001010f4  0x5008 Xcpt #DF errcd=0x0
00:00:02.951749       76054: 0x000006082d7dbba6/-1780     00000000001010f4  0x500d Xcpt #GP errcd=0x0
00:00:02.951752       76053: 0x000006082d7daba7/-4095     00000000001010f4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951755       76052: 0x000006082d7d6210/-18839    000000000010104b  0x0405 0x0405
00:00:02.951758       76051: 0x000006082d7d5af6/-1818     000000000010103c  0x0405 0x0405
00:00:02.951761       76050: 0x000006082d7d53e2/-1812     0000000000101035  0x0405 0x0405
00:00:02.951764       76049: 0x000006082d7d4ce2/-1792     000000000010102e  0x0405 0x0405
00:00:02.951766       76048: 0x000006082d7d45ce/-1812     0000000000101027  0x0405 0x0405
00:00:02.951769       76047: 0x000006082d7d3e5e/-1904     0000000000101020  0x0405 0x0405
00:00:02.951771       76046: 0x000006082d7d3702/-1884     0000000000101019  0x0405 0x0405
00:00:02.951774       76045: 0x000006082d7d2f4b/-1975     0000000000101012  0x0405 0x0405
00:00:02.951777       76044: 0x000006082d7d27a9/-1954     000000000010100b  0x0405 0x0405
00:00:02.951779       76043: 0x000006082d7d1a67/-3394     0000000000101008  0x0405 0x0405
00:00:02.951782       76042: 0x000006082d7b4364/-120579   00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951785       76041: 0x000006082d7b37b1/-2995     00000000000f181a  0x0404 0x0404
00:00:02.951787       76040: 0x000006082d7b2f11/-2208     00000000000f1815  0x0405 0x0405
00:00:02.951790       76039: 0x000006082d7b2679/-2200     00000000000f181a  0x0404 0x0404
00:00:02.951792       76038: 0x000006082d7b1dca/-2223     00000000000f1815  0x0405 0x0405
00:00:02.951795       76037: 0x000006082d7b152f/-2203     00000000000f181a  0x0404 0x0404
00:00:02.951798       76036: 0x000006082d7b0c7a/-2229     00000000000f1815  0x0405 0x0405
00:00:02.951800       76035: 0x000006082d7b03ee/-2188     00000000000f181a  0x0404 0x0404
00:00:02.951803       76034: 0x000006082d7afb3f/-2223     00000000000f1815  0x0405 0x0405
00:00:02.951805       76033: 0x000006082d7af29e/-2209     00000000000f181a  0x0404 0x0404
00:00:02.951808       76032: 0x000006082d7ae9e6/-2232     00000000000f1815  0x0405 0x0405
00:00:02.951811       76031: 0x000006082d7ae122/-2244     00000000000f181a  0x0404 0x0404
00:00:02.951813       76030: 0x000006082d7ad49a/-3208     00000000000f1815  0x0405 0x0405
00:00:02.951816       76029: 0x000006082d7ac00f/-5259     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951818       76028: 0x000006082d7aa5bd/-6738     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951821       76027: 0x000006082d7a9a27/-2966     00000000000f181a  0x0404 0x0404
00:00:02.951824       76026: 0x000006082d7a9181/-2214     00000000000f1815  0x0405 0x0405
00:00:02.951826       76025: 0x000006082d7a88f8/-2185     00000000000f181a  0x0404 0x0404
00:00:02.951829       76024: 0x000006082d7a8069/-2191     00000000000f1815  0x0405 0x0405
00:00:02.951832       76023: 0x000006082d7a77c3/-2214     00000000000f181a  0x0404 0x0404
00:00:02.951834       76022: 0x000006082d7a6f31/-2194     00000000000f1815  0x0405 0x0405
00:00:02.951837       76021: 0x000006082d7a66a8/-2185     00000000000f181a  0x0404 0x0404
00:00:02.951839       76020: 0x000006082d7a5dfc/-2220     00000000000f1815  0x0405 0x0405
00:00:02.951842       76019: 0x000006082d7a556a/-2194     00000000000f181a  0x0404 0x0404
00:00:02.951845       76018: 0x000006082d7a4cc7/-2211     00000000000f1815  0x0405 0x0405
00:00:02.951848       76017: 0x000006082d7a43fd/-2250     00000000000f181a  0x0404 0x0404
00:00:02.951851       76016: 0x000006082d7a375a/-3235     00000000000f1815  0x0405 0x0405
00:00:02.951854       76015: 0x000006082d7a22e3/-5239     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951856       76014: 0x000006082d7a0897/-6732     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951859       76013: 0x000006082d79fcea/-2989     00000000000f181a  0x0404 0x0404
00:00:02.951861       76012: 0x000006082d79f438/-2226     00000000000f1815  0x0405 0x0405
00:00:02.951864       76011: 0x000006082d79ebac/-2188     00000000000f181a  0x0404 0x0404
00:00:02.951867       76010: 0x000006082d79e300/-2220     00000000000f1815  0x0405 0x0405
00:00:02.951869       76009: 0x000006082d79da71/-2191     00000000000f181a  0x0404 0x0404
00:00:02.951872       76008: 0x000006082d79d1df/-2194     00000000000f1815  0x0405 0x0405
00:00:02.951874       76007: 0x000006082d79c942/-2205     00000000000f181a  0x0404 0x0404
00:00:02.951877       76006: 0x000006082d79c0aa/-2200     00000000000f1815  0x0405 0x0405
00:00:02.951880       76005: 0x000006082d79b818/-2194     00000000000f181a  0x0404 0x0404
00:00:02.951882       76004: 0x000006082d79af6c/-2220     00000000000f1815  0x0405 0x0405
00:00:02.951885       76003: 0x000006082d79a6a8/-2244     00000000000f181a  0x0404 0x0404
00:00:02.951887       76002: 0x000006082d799a1a/-3214     00000000000f1815  0x0405 0x0405
00:00:02.951890       76001: 0x000006082d7985a3/-5239     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951893       76000: 0x000006082d796b63/-6720     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951895       75999: 0x000006082d795fb3/-2992     00000000000f181a  0x0404 0x0404
00:00:02.951898       75998: 0x000006082d79570a/-2217     00000000000f1815  0x0405 0x0405
00:00:02.951900       75997: 0x000006082d794e84/-2182     00000000000f181a  0x0404 0x0404
00:00:02.951903       75996: 0x000006082d7945e3/-2209     00000000000f1815  0x0405 0x0405
00:00:02.951906       75995: 0x000006082d793d40/-2211     00000000000f181a  0x0404 0x0404
00:00:02.951908       75994: 0x000006082d7934a2/-2206     00000000000f1815  0x0405 0x0405
00:00:02.951911       75993: 0x000006082d792c08/-2202     00000000000f181a  0x0404 0x0404
00:00:02.951913       75992: 0x000006082d792370/-2200     00000000000f1815  0x0405 0x0405
00:00:02.951916       75991: 0x000006082d791ac7/-2217     00000000000f181a  0x0404 0x0404
00:00:02.951918       75990: 0x000006082d791235/-2194     00000000000f1815  0x0405 0x0405
00:00:02.951921       75989: 0x000006082d790965/-2256     00000000000f181a  0x0404 0x0404
00:00:02.951924       75988: 0x000006082d78fcb1/-3252     00000000000f1815  0x0405 0x0405
00:00:02.951926       75987: 0x000006082d78e83d/-5236     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951929       75986: 0x000006082d78cdeb/-6738     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951932       75985: 0x000006082d78c23b/-2992     00000000000f181a  0x0404 0x0404
00:00:02.951934       75984: 0x000006082d78b9b2/-2185     00000000000f1815  0x0405 0x0405
00:00:02.951937       75983: 0x000006082d78b112/-2208     00000000000f181a  0x0404 0x0404
00:00:02.951939       75982: 0x000006082d78a87d/-2197     00000000000f1815  0x0405 0x0405
00:00:02.951942       75981: 0x000006082d789fdf/-2206     00000000000f181a  0x0404 0x0404
00:00:02.951945       75980: 0x000006082d789739/-2214     00000000000f1815  0x0405 0x0405
00:00:02.951947       75979: 0x000006082d788ead/-2188     00000000000f181a  0x0404 0x0404
00:00:02.951950       75978: 0x000006082d788618/-2197     00000000000f1815  0x0405 0x0405
00:00:02.951952       75977: 0x000006082d787d75/-2211     00000000000f181a  0x0404 0x0404
00:00:02.951955       75976: 0x000006082d7874e3/-2194     00000000000f1815  0x0405 0x0405
00:00:02.951958       75975: 0x000006082d786c13/-2256     00000000000f181a  0x0404 0x0404
00:00:02.951960       75974: 0x000006082d785f7d/-3222     00000000000f1815  0x0405 0x0405
00:00:02.951964       75973: 0x000006082d784b06/-5239     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951966       75972: 0x000006082d78312a/-6620     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.951969       75971: 0x000006082d782586/-2980     00000000000f181a  0x0404 0x0404
00:00:02.951972       75970: 0x000006082d781cf1/-2197     00000000000f1815  0x0405 0x0405
00:00:02.951974       75969: 0x000006082d78146b/-2182     00000000000f181a  0x0404 0x0404
00:00:02.951977       75968: 0x000006082d780bc8/-2211     00000000000f1815  0x0405 0x0405
00:00:02.951980       75967: 0x000006082d78032d/-2203     00000000000f181a  0x0404 0x0404
00:00:02.951982       75966: 0x000006082d77fa8a/-2211     00000000000f1815  0x0405 0x0405
00:00:02.951985       75965: 0x000006082d77f201/-2185     00000000000f181a  0x0404 0x0404
00:00:02.951987       75964: 0x000006082d77e960/-2209     00000000000f1815  0x0405 0x0405
00:00:02.951990       75963: 0x000006082d77e0bd/-2211     00000000000f181a  0x0404 0x0404
00:00:02.951993       75962: 0x000006082d77d81f/-2206     00000000000f1815  0x0405 0x0405
00:00:02.951995       75961: 0x000006082d77cf47/-2264     00000000000f181a  0x0404 0x0404
00:00:02.951998       75960: 0x000006082d77c29e/-3241     00000000000f1815  0x0405 0x0405
00:00:02.952000       75959: 0x000006082d77ae13/-5259     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952003       75958: 0x000006082d779440/-6611     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952006       75957: 0x000006082d77889f/-2977     00000000000f181a  0x0404 0x0404
00:00:02.952008       75956: 0x000006082d777ffe/-2209     00000000000f1815  0x0405 0x0405
00:00:02.952011       75955: 0x000006082d77776f/-2191     00000000000f181a  0x0404 0x0404
00:00:02.952013       75954: 0x000006082d776ecc/-2211     00000000000f1815  0x0405 0x0405
00:00:02.952016       75953: 0x000006082d77663d/-2191     00000000000f181a  0x0404 0x0404
00:00:02.952019       75952: 0x000006082d775dab/-2194     00000000000f1815  0x0405 0x0405
00:00:02.952021       75951: 0x000006082d775508/-2211     00000000000f181a  0x0404 0x0404
00:00:02.952024       75950: 0x000006082d774c76/-2194     00000000000f1815  0x0405 0x0405
00:00:02.952027       75949: 0x000006082d7743d6/-2208     00000000000f181a  0x0404 0x0404
00:00:02.952029       75948: 0x000006082d773b38/-2206     00000000000f1815  0x0405 0x0405
00:00:02.952032       75947: 0x000006082d773271/-2247     00000000000f181a  0x0404 0x0404
00:00:02.952034       75946: 0x000006082d7725f8/-3193     00000000000f1815  0x0405 0x0405
00:00:02.952037       75945: 0x000006082d771152/-5286     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952040       75944: 0x000006082d76f776/-6620     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952043       75943: 0x000006082d76ebc0/-2998     00000000000f181a  0x0404 0x0404
00:00:02.952045       75942: 0x000006082d76e31d/-2211     00000000000f1815  0x0405 0x0405
00:00:02.952048       75941: 0x000006082d76da94/-2185     00000000000f181a  0x0404 0x0404
00:00:02.952050       75940: 0x000006082d76d1e2/-2226     00000000000f1815  0x0405 0x0405
00:00:02.952053       75939: 0x000006082d76c953/-2191     00000000000f181a  0x0404 0x0404
00:00:02.952055       75938: 0x000006082d76c0b2/-2209     00000000000f1815  0x0405 0x0405
00:00:02.952058       75937: 0x000006082d76b81e/-2196     00000000000f181a  0x0404 0x0404
00:00:02.952061       75936: 0x000006082d76af86/-2200     00000000000f1815  0x0405 0x0405
00:00:02.952063       75935: 0x000006082d76a6ee/-2200     00000000000f181a  0x0404 0x0404
00:00:02.952066       75934: 0x000006082d769e51/-2205     00000000000f1815  0x0405 0x0405
00:00:02.952068       75933: 0x000006082d769584/-2253     00000000000f181a  0x0404 0x0404
00:00:02.952071       75932: 0x000006082d7688f6/-3214     00000000000f1815  0x0405 0x0405
00:00:02.952073       75931: 0x000006082d767459/-5277     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952076       75930: 0x000006082d76581c/-7229     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952079       75929: 0x000006082d764c7b/-2977     00000000000f181a  0x0404 0x0404
00:00:02.952081       75928: 0x000006082d7643e0/-2203     00000000000f1815  0x0405 0x0405
00:00:02.952084       75927: 0x000006082d763b37/-2217     00000000000f181a  0x0404 0x0404
00:00:02.952086       75926: 0x000006082d7632a5/-2194     00000000000f1815  0x0405 0x0405
00:00:02.952089       75925: 0x000006082d762a02/-2211     00000000000f181a  0x0404 0x0404
00:00:02.952092       75924: 0x000006082d762164/-2206     00000000000f1815  0x0405 0x0405
00:00:02.952094       75923: 0x000006082d7618cf/-2197     00000000000f181a  0x0404 0x0404
00:00:02.952097       75922: 0x000006082d761035/-2202     00000000000f1815  0x0405 0x0405
00:00:02.952099       75921: 0x000006082d7607a0/-2197     00000000000f181a  0x0404 0x0404
00:00:02.952102       75920: 0x000006082d75ff0e/-2194     00000000000f1815  0x0405 0x0405
00:00:02.952104       75919: 0x000006082d75f62a/-2276     00000000000f181a  0x0404 0x0404
00:00:02.952107       75918: 0x000006082d75e99f/-3211     00000000000f1815  0x0405 0x0405
00:00:02.952109       75917: 0x000006082d75d4e1/-5310     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952112       75916: 0x000006082d75b727/-7610     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952115       75915: 0x000006082d75ab48/-3039     00000000000f181a  0x0404 0x0404
00:00:02.952117       75914: 0x000006082d75a2a7/-2209     00000000000f1815  0x0405 0x0405
00:00:02.952120       75913: 0x000006082d759a24/-2179     00000000000f181a  0x0404 0x0404
00:00:02.952123       75912: 0x000006082d759181/-2211     00000000000f1815  0x0405 0x0405
00:00:02.952125       75911: 0x000006082d7588c6/-2235     00000000000f181a  0x0404 0x0404
00:00:02.952128       75910: 0x000006082d75802b/-2203     00000000000f1815  0x0405 0x0405
00:00:02.952130       75909: 0x000006082d7577a5/-2182     00000000000f181a  0x0404 0x0404
00:00:02.952133       75908: 0x000006082d756ef3/-2226     00000000000f1815  0x0405 0x0405
00:00:02.952135       75907: 0x000006082d756652/-2209     00000000000f181a  0x0404 0x0404
00:00:02.952138       75906: 0x000006082d755d9d/-2229     00000000000f1815  0x0405 0x0405
00:00:02.952141       75905: 0x000006082d7554d6/-2247     00000000000f181a  0x0404 0x0404
00:00:02.952143       75904: 0x000006082d75481f/-3255     00000000000f1815  0x0405 0x0405
00:00:02.952146       75903: 0x000006082d753393/-5260     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952148       75902: 0x000006082d7519be/-6613     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952151       75901: 0x000006082d750df9/-3013     00000000000f181a  0x0404 0x0404
00:00:02.952154       75900: 0x000006082d75053b/-2238     00000000000f1815  0x0405 0x0405
00:00:02.952156       75899: 0x000006082d74fcac/-2191     00000000000f181a  0x0404 0x0404
00:00:02.952159       75898: 0x000006082d74f3fa/-2226     00000000000f1815  0x0405 0x0405
00:00:02.952161       75897: 0x000006082d74eb53/-2215     00000000000f181a  0x0404 0x0404
00:00:02.952164       75896: 0x000006082d74e2b3/-2208     00000000000f1815  0x0405 0x0405
00:00:02.952167       75895: 0x000006082d74da18/-2203     00000000000f181a  0x0404 0x0404
00:00:02.952169       75894: 0x000006082d74d175/-2211     00000000000f1815  0x0405 0x0405
00:00:02.952172       75893: 0x000006082d74c8da/-2203     00000000000f181a  0x0404 0x0404
00:00:02.952174       75892: 0x000006082d74c02b/-2223     00000000000f1815  0x0405 0x0405
00:00:02.952177       75891: 0x000006082d74b761/-2250     00000000000f181a  0x0404 0x0404
00:00:02.952180       75890: 0x000006082d74aad3/-3214     00000000000f1815  0x0405 0x0405
00:00:02.952182       75889: 0x000006082d749642/-5265     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952185       75888: 0x000006082d747c9b/-6567     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952187       75887: 0x000006082d7470c5/-3030     00000000000f181a  0x0404 0x0404
00:00:02.952190       75886: 0x000006082d746827/-2206     00000000000f1815  0x0405 0x0405
00:00:02.952193       75885: 0x000006082d745f98/-2191     00000000000f181a  0x0404 0x0404
00:00:02.952195       75884: 0x000006082d7456e6/-2226     00000000000f1815  0x0405 0x0405
00:00:02.952198       75883: 0x000006082d744e40/-2214     00000000000f181a  0x0404 0x0404
00:00:02.952200       75882: 0x000006082d7445a5/-2203     00000000000f1815  0x0405 0x0405
00:00:02.952203       75881: 0x000006082d743d1c/-2185     00000000000f181a  0x0404 0x0404
00:00:02.952205       75880: 0x000006082d743479/-2211     00000000000f1815  0x0405 0x0405
00:00:02.952208       75879: 0x000006082d742bd5/-2212     00000000000f181a  0x0404 0x0404
00:00:02.952210       75878: 0x000006082d742338/-2205     00000000000f1815  0x0405 0x0405
00:00:02.952213       75877: 0x000006082d741a6e/-2250     00000000000f181a  0x0404 0x0404
00:00:02.952216       75876: 0x000006082d740dba/-3252     00000000000f1815  0x0405 0x0405
00:00:02.952218       75875: 0x000006082d73f931/-5257     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952221       75874: 0x000006082d73df76/-6587     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952224       75873: 0x000006082d73d3c0/-2998     00000000000f181a  0x0404 0x0404
00:00:02.952226       75872: 0x000006082d73cb2b/-2197     00000000000f1815  0x0405 0x0405
00:00:02.952229       75871: 0x000006082d73c288/-2211     00000000000f181a  0x0404 0x0404
00:00:02.952231       75870: 0x000006082d73b9e4/-2212     00000000000f1815  0x0405 0x0405
00:00:02.952234       75869: 0x000006082d73b13e/-2214     00000000000f181a  0x0404 0x0404
00:00:02.952236       75868: 0x000006082d73a894/-2218     00000000000f1815  0x0405 0x0405
00:00:02.952239       75867: 0x000006082d739ffd/-2199     00000000000f181a  0x0404 0x0404
00:00:02.952242       75866: 0x000006082d739759/-2212     00000000000f1815  0x0405 0x0405
00:00:02.952244       75865: 0x000006082d738ead/-2220     00000000000f181a  0x0404 0x0404
00:00:02.952247       75864: 0x000006082d738615/-2200     00000000000f1815  0x0405 0x0405
00:00:02.952249       75863: 0x000006082d737d37/-2270     00000000000f181a  0x0404 0x0404
00:00:02.952252       75862: 0x000006082d7370ac/-3211     00000000000f1815  0x0405 0x0405
00:00:02.952254       75861: 0x000006082d735c17/-5269     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952257       75860: 0x000006082d734259/-6590     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952260       75859: 0x000006082d7336a6/-2995     00000000000f181a  0x0404 0x0404
00:00:02.952262       75858: 0x000006082d732dfa/-2220     00000000000f1815  0x0405 0x0405
00:00:02.952265       75857: 0x000006082d73256e/-2188     00000000000f181a  0x0404 0x0404
00:00:02.952268       75856: 0x000006082d731cc5/-2217     00000000000f1815  0x0405 0x0405
00:00:02.952270       75855: 0x000006082d731424/-2209     00000000000f181a  0x0404 0x0404
00:00:02.952273       75854: 0x000006082d730b7e/-2214     00000000000f1815  0x0405 0x0405
00:00:02.952275       75853: 0x000006082d7302ef/-2191     00000000000f181a  0x0404 0x0404
00:00:02.952278       75852: 0x000006082d72fa3a/-2229     00000000000f1815  0x0405 0x0405
00:00:02.952281       75851: 0x000006082d72f1a5/-2197     00000000000f181a  0x0404 0x0404
00:00:02.952283       75850: 0x000006082d72e8f9/-2220     00000000000f1815  0x0405 0x0405
00:00:02.952286       75849: 0x000006082d72e029/-2256     00000000000f181a  0x0404 0x0404
00:00:02.952288       75848: 0x000006082d72d378/-3249     00000000000f1815  0x0405 0x0405
00:00:02.952291       75847: 0x000006082d72beef/-5257     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952293       75846: 0x000006082d72a531/-6590     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952296       75845: 0x000006082d72997b/-2998     00000000000f181a  0x0404 0x0404
00:00:02.952299       75844: 0x000006082d7290c9/-2226     00000000000f1815  0x0405 0x0405
00:00:02.952301       75843: 0x000006082d728831/-2200     00000000000f181a  0x0404 0x0404
00:00:02.952304       75842: 0x000006082d727f82/-2223     00000000000f1815  0x0405 0x0405
00:00:02.952306       75841: 0x000006082d7276ea/-2200     00000000000f181a  0x0404 0x0404
00:00:02.952309       75840: 0x000006082d726e44/-2214     00000000000f1815  0x0405 0x0405
00:00:02.952311       75839: 0x000006082d7265a3/-2209     00000000000f181a  0x0404 0x0404
00:00:02.952314       75838: 0x000006082d725d03/-2208     00000000000f1815  0x0405 0x0405
00:00:02.952317       75837: 0x000006082d72546b/-2200     00000000000f181a  0x0404 0x0404
00:00:02.952319       75836: 0x000006082d724bbc/-2223     00000000000f1815  0x0405 0x0405
00:00:02.952322       75835: 0x000006082d7242f2/-2250     00000000000f181a  0x0404 0x0404
00:00:02.952324       75834: 0x000006082d723664/-3214     00000000000f1815  0x0405 0x0405
00:00:02.952327       75833: 0x000006082d7221de/-5254     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952330       75832: 0x000006082d720806/-6616     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952332       75831: 0x000006082d71fc44/-3010     00000000000f181a  0x0404 0x0404
00:00:02.952335       75830: 0x000006082d71f3a3/-2209     00000000000f1815  0x0405 0x0405
00:00:02.952338       75829: 0x000006082d71eb0c/-2199     00000000000f181a  0x0404 0x0404
00:00:02.952340       75828: 0x000006082d71e265/-2215     00000000000f1815  0x0405 0x0405
00:00:02.952343       75827: 0x000006082d71d9c5/-2208     00000000000f181a  0x0404 0x0404
00:00:02.952345       75826: 0x000006082d71d121/-2212     00000000000f1815  0x0405 0x0405
00:00:02.952348       75825: 0x000006082d71c881/-2208     00000000000f181a  0x0404 0x0404
00:00:02.952351       75824: 0x000006082d71bfe3/-2206     00000000000f1815  0x0405 0x0405
00:00:02.952353       75823: 0x000006082d71b73a/-2217     00000000000f181a  0x0404 0x0404
00:00:02.952356       75822: 0x000006082d71aea5/-2197     00000000000f1815  0x0405 0x0405
00:00:02.952358       75821: 0x000006082d71a5cf/-2262     00000000000f181a  0x0404 0x0404
00:00:02.952361       75820: 0x000006082d71991e/-3249     00000000000f1815  0x0405 0x0405
00:00:02.952364       75819: 0x000006082d718419/-5381     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952366       75818: 0x000006082d716a14/-6661     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952369       75817: 0x000006082d715e5b/-3001     00000000000f181a  0x0404 0x0404
00:00:02.952372       75816: 0x000006082d7155b8/-2211     00000000000f1815  0x0405 0x0405
00:00:02.952374       75815: 0x000006082d714d1a/-2206     00000000000f181a  0x0404 0x0404
00:00:02.952377       75814: 0x000006082d714474/-2214     00000000000f1815  0x0405 0x0405
00:00:02.952379       75813: 0x000006082d713bdc/-2200     00000000000f181a  0x0404 0x0404
00:00:02.952382       75812: 0x000006082d713318/-2244     00000000000f1815  0x0405 0x0405
00:00:02.952385       75811: 0x000006082d712a8c/-2188     00000000000f181a  0x0404 0x0404
00:00:02.952387       75810: 0x000006082d7121ce/-2238     00000000000f1815  0x0405 0x0405
00:00:02.952390       75809: 0x000006082d71190d/-2241     00000000000f181a  0x0404 0x0404
00:00:02.952392       75808: 0x000006082d711067/-2214     00000000000f1815  0x0405 0x0405
00:00:02.952395       75807: 0x000006082d710791/-2262     00000000000f181a  0x0404 0x0404
00:00:02.952397       75806: 0x000006082d70fa87/-3338     00000000000f1815  0x0405 0x0405
00:00:02.952400       75805: 0x000006082d70e5a3/-5348     0000000000008370  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952403       75804: 0x000006082d70c830/-7539     00000000000082e4  0x101c VMX_EXIT_MOV_CRX - 28 - Control-register accesses.
00:00:02.952405       75803: 0x000006082d70bc36/-3066     00000000000f181a  0x0404 0x0404
00:00:02.952408       75802: 0x000006082d70b372/-2244     00000000000f1815  0x0405 0x0405
00:00:02.952410       75801: 0x000006082d70aadd/-2197     00000000000f181a  0x0404 0x0404
00:00:02.952418       75800: 0x000006082d70a204/-2265     00000000000f1815  0x0405 0x0405
00:00:02.952422 CPU[1]: VM-exit history: empty
00:00:02.952436 CPU[2]: VM-exit history: empty
00:00:02.952445 CPU[3]: VM-exit history: empty
00:00:02.952454 !!
00:00:02.952455 !! {fflags}
00:00:02.952456 !!
00:00:02.952457 Global FFs: 0x400
00:00:02.952459     CHECK_VM_STATE
00:00:02.952461   Groups:
00:00:02.952461     EXTERNAL_SUSPENDED, EXTERNAL_HALTED, HIGH_PRIORITY_PRE, NORMAL_PRIORITY_POST, ALL_REM
00:00:02.952464 CPU 0 FFs: 0x0
00:00:02.952466 CPU 1 FFs: 0x10000
00:00:02.952468     PGM_SYNC_CR3
00:00:02.952469   Groups:
00:00:02.952469     HIGH_PRIORITY_PRE, HIGH_PRIORITY_PRE_RAW
00:00:02.952470 CPU 2 FFs: 0x10000
00:00:02.952471     PGM_SYNC_CR3
00:00:02.952472   Groups:
00:00:02.952472     HIGH_PRIORITY_PRE, HIGH_PRIORITY_PRE_RAW
00:00:02.952475 CPU 3 FFs: 0x10000
00:00:02.952476     PGM_SYNC_CR3
00:00:02.952476   Groups:
00:00:02.952477     HIGH_PRIORITY_PRE, HIGH_PRIORITY_PRE_RAW
00:00:02.952478 !!
00:00:02.952478 !! {gdt}
00:00:02.952479 !!
00:00:02.952482 Guest GDT (GCAddr=0000000000101130 limit=17):
00:00:02.952489 0008 - 0000ffff 00cf9b00 - base=00000000 limit=ffffffff dpl=0 CodeER Accessed Present Page 32-bit
00:00:02.952491 0010 - 0000ffff 00cf9300 - base=00000000 limit=ffffffff dpl=0 DataRW Accessed Present Page 32-bit
00:00:02.952492 !!
00:00:02.952493 !! {guestprops}
00:00:02.952493 !!
00:00:02.952495 /VirtualBox/HostInfo/GUI/LanguageID: 'uk_UA', 1783450264479568000 (RDONLYGUEST)
00:00:02.952498 /VirtualBox/GuestAdd/GuiOnFocus: '1', 1783450264498161000 (TRANSIENT, RDONLYGUEST)
00:00:02.952501 /VirtualBox/HostInfo/VBoxVerExt: '7.2.12', 1783450263359503001 (TRANSIENT, RDONLYGUEST)
00:00:02.952503 /VirtualBox/VMInfo/ResumeCounter: '0', 1783450263300877001 (TRANSIENT, RDONLYGUEST)
00:00:02.952505 /VirtualBox/HostGuest/SysprepExec: '', 1783450263300877002 (TRANSIENT, RDONLYGUEST)
00:00:02.952508 /VirtualBox/HostGuest/SysprepArgs: '', 1783450263300877003 (TRANSIENT, RDONLYGUEST)
00:00:02.952510 /VirtualBox/VMInfo/ResetCounter: '0', 1783450263300877000 (TRANSIENT, RDONLYGUEST)
00:00:02.952512 /VirtualBox/HostInfo/VBoxRev: '174389', 1783450263359503002 (TRANSIENT, RDONLYGUEST)
00:00:02.952515 /VirtualBox/HostInfo/VBoxVer: '7.2.12', 1783450263359503000 (TRANSIENT, RDONLYGUEST)
00:00:02.952518 !!
00:00:02.952519 !! {hm}
00:00:02.952519 !!
00:00:02.952521 CPU[0]: VT-x info:
00:00:02.952522   HM error           = 0x0 (0)
00:00:02.952523   rcLastExitToR3     = VINF_EM_TRIPLE_FAULT
00:00:02.952527   Guest VMCS active
00:00:02.952527     Real-on-v86 active = false
00:00:02.952529 CPU[1]: VT-x info:
00:00:02.952530   HM error           = 0x0 (0)
00:00:02.952531   rcLastExitToR3     = VINF_SUCCESS
00:00:02.952533   Guest VMCS active
00:00:02.952533     Real-on-v86 active = false
00:00:02.952543 CPU[2]: VT-x info:
00:00:02.952544   HM error           = 0x0 (0)
00:00:02.952545   rcLastExitToR3     = VINF_SUCCESS
00:00:02.952546   Guest VMCS active
00:00:02.952546     Real-on-v86 active = false
00:00:02.952571 CPU[3]: VT-x info:
00:00:02.952572   HM error           = 0x0 (0)
00:00:02.952573   rcLastExitToR3     = VINF_SUCCESS
00:00:02.952574   Guest VMCS active
00:00:02.952574     Real-on-v86 active = false
00:00:02.952595 !!
00:00:02.952596 !! {hmeventpending}
00:00:02.952596 !!
00:00:02.952598 CPU[0]: HM event (fPending=false)
00:00:02.952600 CPU[1]: HM event (fPending=false)
00:00:02.952610 CPU[2]: HM event (fPending=false)
00:00:02.952633 CPU[3]: HM event (fPending=false)
00:00:02.952644 !!
00:00:02.952644 !! {ioapic}
00:00:02.952645 !!
00:00:02.952646 I/O APIC at 0xfec00000:
00:00:02.952647   ID                      = 0x0
00:00:02.952649     ID                      = 0x0
00:00:02.952649   Version                 = 0x170020
00:00:02.952651     Version                 = 0x20
00:00:02.952651     Pin Assert Reg. Support = false
00:00:02.952652     Max. Redirection Entry  = 23
00:00:02.952653   Current index           = 0x0
00:00:02.952654   I/O Redirection Table and IRR:
00:00:02.952654   idx dst_mode dst_addr mask irr trigger rirr polar dlvr_st dlvr_mode vector rte
00:00:02.952655   ---------------------------------------------------------------------------------------------
00:00:02.952656    00     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952659    01     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952662    02     phys       00    1   1    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952665    03     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952667    04     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952669    05     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952672    06     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952674    07     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952676    08     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952679    09     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952681    10     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952683    11     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952686    12     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952688    13     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952691    14     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952693    15     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952695    16     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952698    17     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952700    18     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952702    19     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952704    20     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952707    21     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952709    22     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952711    23     phys       00    1   0    edge    0 acthi    idle     fixed      0 (0000000000010000)
00:00:02.952714 !!
00:00:02.952715 !! {ioport}
00:00:02.952715 !!
00:00:02.952716 I/O port registrations: 68 (73 allocated)
00:00:02.952717  ## Ctx    Ports Mapping   PCI    Description
00:00:02.952718   0 R3     0010  00f0-00ff        Math Co-Processor (DOS/OS2 mode)
00:00:02.952720   1 R3     0001  0092-0092        PS/2 system control port A (A20 and more)
00:00:02.952722   2 R3     0004  0400-0403        Bochs PC BIOS - Panic & Debug
00:00:02.952724   3 R3     0001  040f-040f        PC BIOS - Control
00:00:02.952725   4 R3+0   0001  0cf8-0cf8        i440FX (PCI)
00:00:02.952727   5 R3+0   0004  0cfc-0cff        i440FX (PCI)
00:00:02.952729   6 R3     0001  0410-0410        i440FX (Fake PCI BIOS trigger)
00:00:02.952730   7 R3+0   0001  0060-0060        PC Keyboard - Data
00:00:02.952732   8 R3+0   0001  0064-0064        PC Keyboard - Command / Status
00:00:02.952733   9 R3+0   0002  0020-0021        i8259 PIC #0
00:00:02.952735  10 R3+0   0002  00a0-00a1        i8259 PIC #1
00:00:02.952737  11 R3+0   0001  04d0-04d0        i8259 PIC #0 - elcr
00:00:02.952738  12 R3+0   0001  04d1-04d1        i8259 PIC #1 - elcr
00:00:02.952740  13 R3+0   0004  0040-0043        i8254 Programmable Interval Timer
00:00:02.952742  14 R3+0   0001  0061-0061        PC Speaker
00:00:02.952743  15 R3+0   0004  0070-0073        MC146818 RTC/CMOS
00:00:02.952744  16 R3+0   0008  0000-0007        DMA8 Address
00:00:02.952746  17 R3+0   0010  00c0-00cf        DMA16 Address
00:00:02.952748  18 R3+0   0008  0008-000f        DMA8 Control
00:00:02.952749  19 R3+0   0010  00d0-00df        DMA16 Control
00:00:02.952751  20 R3+0   0008  0080-0087        DMA8 Page
00:00:02.952752  21 R3+0   0008  0088-008f        DMA16 Page
00:00:02.952753  22 R3     0001  0504-0504        VMMDev backdoor logging
00:00:02.952785  23 R3     0001  0505-0505        VMMDev timesync backdoor
00:00:02.952787  24 R3     0001  d040-d040 pci0/0 VMMDev Request Handler
00:00:02.952789  25 R3+0   0001  d048-d048 pci0/1 VMMDev Fast R0/RC Requests
00:00:02.952791  26 R3+0   0002  03c0-03c1        VGA - Attribute Controller
00:00:02.952793  27 R3+0   0001  03c2-03c2        VGA - MSR / ST00
00:00:02.952794  28 R3+0   0001  03c3-03c3        VGA - 0x3c3
00:00:02.952796  29 R3+0   0002  03c4-03c5        VGA - Sequencer
00:00:02.952797  30 R3+0   0004  03c6-03c9        VGA - DAC
00:00:02.952799  31 R3+0   0004  03ca-03cd        VGA - Graphics Position
00:00:02.952801  32 R3+0   0002  03ce-03cf        VGA - Graphics Controller
00:00:02.952802  33 R3+0   0002  03b4-03b5        VGA - MDA CRT control
00:00:02.952804  34 R3+0   0001  03ba-03ba        VGA - MDA feature/status
00:00:02.952805  35 R3+0   0002  03d4-03d5        VGA - CGA CRT control
00:00:02.952807  36 R3+0   0001  03da-03da        VGA - CGA Feature / status
00:00:02.952808  37 R3+0   0001  01ce-01ce        VGA - VBE Index
00:00:02.952809  38 R3+0   0001  01cf-01cf        VGA - VBE Data
00:00:02.952811  39 R3     0004  03b0-03b3        VGA - HGSMI host (3b0-3b3)
00:00:02.952812  40 R3     0004  03d0-03d3        VGA - HGSMI guest (3d0-3d3)
00:00:02.952814  41 R3+0   0001  03b7-03b7        VGA BIOS debug/panic
00:00:02.952815  42 R3     0001  03b6-03b6        VBE BIOS Extra Data
00:00:02.952817  43 R3     0001  03b8-03b8        BIOS Logo
00:00:02.952818  44 R3+0   0010  d000-d00f pci0/262144 ATA Bus Master DMA
00:00:02.952821  45 R3+0   0001  01f0-01f0        ATA I/O Base 1 - Data
00:00:02.952822  46 R3+0   0007  01f1-01f7        ATA I/O Base 1 - Other
00:00:02.952824  47 R3+0   0001  03f6-03f6        ATA I/O Base 2
00:00:02.952825  48 R3+0   0001  0170-0170        ATA I/O Base 1 - Data
00:00:02.952828  49 R3+0   0007  0171-0177        ATA I/O Base 1 - Other
00:00:02.952830  50 R3+0   0001  0376-0376        ATA I/O Base 2
00:00:02.952831  51 R3+0   0010  d020-d02f pci0/0 PCnet APROM
00:00:02.952833  52 R3+0   0010  d030-d03f pci0/0 PCnet
00:00:02.952835  53 R3+0   0100  d100-d1ff pci0/0 ICHAC97 NAM
00:00:02.952837  54 R3+0   0040  d200-d23f pci0/65536 ICHAC97 NABM
00:00:02.952839  55 R3     0001  4000-4000        ACPI PM1a Status
00:00:02.952841  56 R3     0001  4002-4002        ACPI PM1a Enable
00:00:02.952843  57 R3     0001  4004-4004        ACPI PM1a Control
00:00:02.952844  58 R3+0   0001  4008-4008        ACPI PM Timer
00:00:02.952845  59 R3     0001  4020-4020        ACPI GPE0 Status
00:00:02.952847  60 R3     0001  4021-4021        ACPI GPE0 Enable
00:00:02.952848  61 R3     0010  4100-410f        SMBus
00:00:02.952850  62 R3     0001  442e-442e        ACPI SMI
00:00:02.952851  63 R3     0001  4040-4040        ACPI Battery status index
00:00:02.952852  64 R3     0001  4044-4044        ACPI Battery status data
00:00:02.952854  65 R3     0001  4048-4048        ACPI system info index
00:00:02.952855  66 R3     0001  404c-404c        ACPI system info data
00:00:02.952856  67 R3     0001  4050-4050        ACPI Reset
00:00:02.952858 !!
00:00:02.952858 !! {irqroute}
00:00:02.952859 !!
00:00:02.952860 PCI interrupt router at: 00:01:0
00:00:02.952862 PIRQA -> IRQ11
00:00:02.952862 PIRQB -> IRQ10
00:00:02.952863 PIRQC -> IRQ9
00:00:02.952864 PIRQD -> IRQ11
00:00:02.952865 !!
00:00:02.952865 !! {itlb}
00:00:02.952865 !!
00:00:02.952869 000: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952874 001: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952877 002: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952880 003: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952882 004: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952884 005: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952887 006: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952889 007: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952891 008: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952893 009: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952896 00a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952898 00b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952900 00c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952902 00d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952905 00e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952907 00f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952910 010: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952912 011: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952914 012: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952916 013: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952919 014: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952921 015: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952923 016: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952926 017: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952928 018: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952930 019: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952933 01a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952935 01b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952938 01c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952940 01d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952943 01e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952945 01f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952947 020: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952950 021: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952952 022: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952954 023: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952957 024: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952959 025: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952961 026: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952963 027: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952966 028: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952968 029: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952970 02a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952972 02b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952975 02c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952977 02d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952980 02e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952982 02f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952984 030: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952986 031: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.952989 032: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.952991 033: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953020 034: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953024 035: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953026 036: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953030 037: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953032 038: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953036 039: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953038 03a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953041 03b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953044 03c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953047 03d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953050 03e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953053 03f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953056 040: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953059 041: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953062 042: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953065 043: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953068 044: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953071 045: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953073 046: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953076 047: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953079 048: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953082 049: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953085 04a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953088 04b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953091 04c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953094 04d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953096 04e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953099 04f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953102 050: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953105 051: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953108 052: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953111 053: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953114 054: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953117 055: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953120 056: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953123 057: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953126 058: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953129 059: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953132 05a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953135 05b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953138 05c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953141 05d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953143 05e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953146 05f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953149 060: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953170 061: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953174 062: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953177 063: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953180 064: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953182 065: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953185 066: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953188 067: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953191 068: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953194 069: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953197 06a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953200 06b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953203 06c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953206 06d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953209 06e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953212 06f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953215 070: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953218 071: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953221 072: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953223 073: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953226 074: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953229 075: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953232 076: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953235 077: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953238 078: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953241 079: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953244 07a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953247 07b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953250 07c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953252 07d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953255 07e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953271 07f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953274 080: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953277 081: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953280 082: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953283 083: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953286 084: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953288 085: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953291 086: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953294 087: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953297 088: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953300 089: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953314 08a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953317 08b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953320 08c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953323 08d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953326 08e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953329 08f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953332 090: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953335 091: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953338 092: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953340 093: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953343 094: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953346 095: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953349 096: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953352 097: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953355 098: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953358 099: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953361 09a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953364 09b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953366 09c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953369 09d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953372 09e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953375 09f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953378 0a0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953381 0a1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953399 0a2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953402 0a3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953405 0a4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953408 0a5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953411 0a6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953419 0a7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953422 0a8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953425 0a9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953428 0aa: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953431 0ab: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953434 0ac: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953437 0ad: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953439 0ae: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953442 0af: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953530 0b0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953534 0b1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953537 0b2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953540 0b3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953543 0b4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953546 0b5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953549 0b6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953552 0b7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953555 0b8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953558 0b9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953561 0ba: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953564 0bb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953566 0bc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953569 0bd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953572 0be: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953575 0bf: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953578 0c0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953581 0c1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953583 0c2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953586 0c3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953589 0c4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953591 0c5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953594 0c6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953596 0c7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953599 0c8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953601 0c9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953603 0ca: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953606 0cb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953608 0cc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953610 0cd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953613 0ce: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953615 0cf: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953617 0d0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953620 0d1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953622 0d2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953625 0d3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953627 0d4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953629 0d5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953632 0d6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953634 0d7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953637 0d8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953639 0d9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953641 0da: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953644 0db: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953646 0dc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953648 0dd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953651 0de: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953653 0df: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953656 0e0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953658 0e1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953660 0e2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953663 0e3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953665 0e4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953667 0e5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953670 0e6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953672 0e7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953674 0e8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953677 0e9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953679 0ea: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953681 0eb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953684 0ec: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953686 0ed: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953688 0ee: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953691 0ef: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953693 0f0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953695 0f1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953698 0f2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953700 0f3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953702 0f4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953705 0f5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953707 0f6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953709 0f7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953712 0f8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953714 0f9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953716 0fa: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953719 0fb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953721 0fc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953723 0fd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953726 0fe: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953728 0ff: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953731 100: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953733 101: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953735 102: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953738 103: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953740 104: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953742 105: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953745 106: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953747 107: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953750 108: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953752 109: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953754 10a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953757 10b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953759 10c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953761 10d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953764 10e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953766 10f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953769 110: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953771 111: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953773 112: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953776 113: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953778 114: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953780 115: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953783 116: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953785 117: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953787 118: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953790 119: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953792 11a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953794 11b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953797 11c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953799 11d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953801 11e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953804 11f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953806 120: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953809 121: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953811 122: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953813 123: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953816 124: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953818 125: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953820 126: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953823 127: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953825 128: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953827 129: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953830 12a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953832 12b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953834 12c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953837 12d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953839 12e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953841 12f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953844 130: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953846 131: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953848 132: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953851 133: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953853 134: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953855 135: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953858 136: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953860 137: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953862 138: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953865 139: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953867 13a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953870 13b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953872 13c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953874 13d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953877 13e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953879 13f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953881 140: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953884 141: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953886 142: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953888 143: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953891 144: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953893 145: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953895 146: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953898 147: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953900 148: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953902 149: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953905 14a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953907 14b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953909 14c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953912 14d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953914 14e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953917 14f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953919 150: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953921 151: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953924 152: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953926 153: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953928 154: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953931 155: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953933 156: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953935 157: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953938 158: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953940 159: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953942 15a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953945 15b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953947 15c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953949 15d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953952 15e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953954 15f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953957 160: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953959 161: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953961 162: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953963 163: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953966 164: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953968 165: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953971 166: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953973 167: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953975 168: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953978 169: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953980 16a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953982 16b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953985 16c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953987 16d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953989 16e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953992 16f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953994 170: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.953996 171: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.953999 172: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954001 173: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954003 174: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954006 175: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954008 176: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954010 177: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954013 178: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954015 179: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954017 17a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954020 17b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954022 17c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954025 17d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954027 17e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954029 17f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954032 180: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954034 181: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954036 182: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954039 183: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954041 184: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954043 185: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954046 186: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954048 187: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954050 188: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954053 189: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954055 18a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954057 18b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954060 18c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954062 18d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954064 18e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954067 18f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954069 190: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954071 191: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954074 192: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954076 193: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954079 194: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954081 195: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954083 196: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954086 197: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954088 198: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954090 199: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954093 19a: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954095 19b: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954097 19c: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954100 19d: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954102 19e: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954104 19f: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954107 1a0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954109 1a1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954111 1a2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954114 1a3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954116 1a4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954118 1a5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954121 1a6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954123 1a7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954126 1a8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954128 1a9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954130 1aa: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954133 1ab: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954135 1ac: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954137 1ad: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954140 1ae: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954142 1af: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954144 1b0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954147 1b1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954149 1b2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954151 1b3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954154 1b4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954156 1b5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954158 1b6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954161 1b7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954163 1b8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954165 1b9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954168 1ba: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954170 1bb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954172 1bc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954175 1bd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954177 1be: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954179 1bf: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954182 1c0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954184 1c1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954187 1c2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954189 1c3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954191 1c4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954194 1c5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954196 1c6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954198 1c7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954201 1c8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954203 1c9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954205 1ca: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954208 1cb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954210 1cc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954212 1cd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954215 1ce: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954217 1cf: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954219 1d0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954222 1d1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954224 1d2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954227 1d3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954229 1d4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954231 1d5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954234 1d6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954236 1d7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954239 1d8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954241 1d9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954243 1da: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954246 1db: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954248 1dc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954250 1dd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954253 1de: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954255 1df: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954258 1e0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954260 1e1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954262 1e2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954265 1e3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954267 1e4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954269 1e5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954272 1e6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954274 1e7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954276 1e8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954279 1e9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954281 1ea: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954283 1eb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954286 1ec: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954288 1ed: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954290 1ee: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954293 1ef: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954295 1f0: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954297 1f1: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954300 1f2: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954302 1f3: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954304 1f4: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954307 1f5: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954309 1f6: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954311 1f7: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954314 1f8: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954316 1f9: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954319 1fa: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954321 1fb: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954323 1fc: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954326 1fd: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954328 1fe: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADS-2M/wr--/M phys-empty
00:00:02.954330 1ff: empty   0x0000000000000000 -> 0000000000000000 / 0000000000000000 / 0x000 RWXADSG2M/wr--/M phys-empty
00:00:02.954334 !!
00:00:02.954334 !! {lbr}
00:00:02.954334 !!
00:00:02.954336 VM not configured to record LBRs for the guest
00:00:02.954338 VM not configured to record LBRs for the guest
00:00:02.954353 VM not configured to record LBRs for the guest
00:00:02.954360 VM not configured to record LBRs for the guest
00:00:02.954366 !!
00:00:02.954366 !! {ldt}
00:00:02.954366 !!
00:00:02.954368 Guest LDT (Sel=0): Null-Selector
00:00:02.954370 !!
00:00:02.954370 !! {mmio}
00:00:02.954370 !!
00:00:02.954371 MMIO registrations: 6 (46 allocated)
00:00:02.954371  ## Ctx    Size Mapping   PCI    Description
00:00:02.954373   0 R3+0   0000000000001000  00000000fee00000-00000000fee00fff        APIC
00:00:02.954376   1 R3+0   0000000000001000  00000000fec00000-00000000fec00fff        I/O APIC
00:00:02.954379   2 R3+0   0000000000020000  00000000000a0000-00000000000bffff        VGA - VGA Video Buffer
00:00:02.954381   3 R3     0000000000001000  00000000f0000000-00000000f0000fff pci0/65536 PCnet
00:00:02.954384   4 R3+0   0000000000001000  00000000f0805000-00000000f0805fff pci0/0 USB EHCI
00:00:02.954387   5 R3+0   0000000000001000  00000000f0804000-00000000f0804fff pci0/0 USB OHCI
00:00:02.954390 !!
00:00:02.954390 !! {nat0}
00:00:02.954391 !!
00:00:02.954391 libslirp Connection Info:
00:00:02.954395   Protocol[State]    FD  Source Address  Port   Dest. Address  Port RecvQ SendQ
00:00:02.954395 libslirp Neighbor Info:
00:00:02.954397   Table  MacAddr            IP Address
00:00:02.954398 libslirp Version String: 4.9.3.0
00:00:02.954399 !!
00:00:02.954399 !! {ohci}
00:00:02.954399 !!
00:00:02.954400 HcControl:          00000200 - CBSR=0 PLE=0 IE=0 CLE=0 BLE=0 HCFS=0x0 IR=0 RWC=1 RWE=0
00:00:02.954403 HcCommandStatus:    00000000 - HCR=0 CLF=0 BLF=0 OCR=0 SOC=0
00:00:02.954404 HcInterruptStatus:  00000040 - SO=0 WDH=0 SF=0 RD=0 UE=0 FNO=0 RHSC=1 OC=0
00:00:02.954406 HcInterruptEnable:  00000000 - SO=0 WDH=0 SF=0 RD=0 UE=0 FNO=0 RHSC=0 OC=0 MIE=0
00:00:02.954408 HcHCCA:             00000000
00:00:02.954409 HcPeriodCurrentED:  00000000
00:00:02.954409 HcControlHeadED:    00000000
00:00:02.954410 HcControlCurrentED: 00000000
00:00:02.954411 HcBulkHeadED:       00000000
00:00:02.954411 HcBulkCurrentED:    00000000
00:00:02.954417 HcDoneHead:         00000000
00:00:02.954418 HcDoneHead:         00000000
00:00:02.954419 HcRhDescriptorA:    0000020c - NDP=12 PSM=0 NPS=1 DT=0 OCPM=0 NOCP=0 POTPGT=0
00:00:02.954420 HcRhDescriptorB:    00000000 - DR=0x00 PPCM=0x00
00:00:02.954421 HcRhStatus:         00000000 - LPS=0 OCI=0 DRWE=0  LPSC=0 OCIC=0 CRWE=0
00:00:02.954422
00:00:02.954423 HcRhPortStatus00: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954424       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954426 HcRhPortStatus01: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954427       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954429 HcRhPortStatus02: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954430       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954431 HcRhPortStatus03: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954432       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954434 HcRhPortStatus04: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954435       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954436 HcRhPortStatus05: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954437       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954439 HcRhPortStatus06: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954440       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954441 HcRhPortStatus07: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954442       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954444 HcRhPortStatus08: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954444       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954446 HcRhPortStatus09: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954447       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954449 HcRhPortStatus10: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954449       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954451 HcRhPortStatus11: CCS=0 PES =0 PSS =0 POCI=0 PRS =0  PPS=0 LSDA=0
00:00:02.954452       00000000 -  CSC=0 PESC=0 PSSC=0 OCIC=0 PRSC=0
00:00:02.954454 !!
00:00:02.954454 !! {pci}
00:00:02.954455 !!
00:00:02.954456 00:00.0 i440FX: 8086-1237 PIIX3
00:00:02.954458         Class base/sub: 0600 (bridge device)
00:00:02.954460         Command: 0000, Status: 0000
00:00:02.954461         Bus master: No
00:00:02.954462 00:01.0 PIIX3: 8086-7000 PIIX3
00:00:02.954464         Class base/sub: 0601 (bridge device)
00:00:02.954465         Command: 0007, Status: 0200
00:00:02.954466         Bus master: Yes
00:00:02.954468 00:01.1 piix3ide: 8086-7111 PIIX3
00:00:02.954469         Class base/sub: 0101 (mass storage controller)
00:00:02.954470         IO region #4: d000..d00f
00:00:02.954472         Command: 0007, Status: 0000
00:00:02.954473         Bus master: Yes
00:00:02.954474 00:02.0 vga: 80ee-beef PIIX3 IRQ10 (INTA#->IRQ18)
00:00:02.954476         Class base/sub: 0300 (display controller)
00:00:02.954487         MMIO32 PREFETCH region #0: e0000000..e7ffffff
00:00:02.954489         Command: 0003, Status: 0000
00:00:02.954490         Bus master: No
00:00:02.954491 00:03.0 pcnet: 1022-2000 PIIX3 IRQ9 (INTA#->IRQ19)
00:00:02.954494         Class base/sub: 0200 (network controller)
00:00:02.954495         IO region #0: d020..d03f
00:00:02.954496         MMIO32 region #1: f0000000..f0000fff
00:00:02.954498         Command: 0007, Status: 0280
00:00:02.954499         Bus master: Yes
00:00:02.954500 00:04.0 VMMDev: 80ee-cafe PIIX3 IRQ11 (INTA#->IRQ20)
00:00:02.954502         Class base/sub: 0880 (base system peripherals)
00:00:02.954503         IO region #0: d040..d05f
00:00:02.954505         MMIO32 region #1: f0400000..f07fffff
00:00:02.954506         MMIO32 PREFETCH region #2: f0800000..f0803fff
00:00:02.954508         Command: 0003, Status: 0000
00:00:02.954509         Bus master: No
00:00:02.954510 00:05.0 ichac97: 8086-2415 PIIX3 IRQ11 (INTA#->IRQ21)
00:00:02.954512         Class base/sub: 0401 (multimedia controller)
00:00:02.954513         IO region #0: d100..d1ff
00:00:02.954514         IO region #1: d200..d23f
00:00:02.954515         Command: 0001, Status: 0280
00:00:02.954516         Bus master: No
00:00:02.954517 00:06.0 usb-ohci: 106b-003f PIIX3 IRQ10 (INTA#->IRQ22)
00:00:02.954519         Class base/sub: 0c03 (serial bus controllers)
00:00:02.954521         MMIO32 region #0: f0804000..f0804fff
00:00:02.954522         Command: 0002, Status: 0010
00:00:02.954523         Bus master: No
00:00:02.954525 00:07.0 acpi: 8086-7113 PIIX3 IRQ9 (INTA#->IRQ23)
00:00:02.954527         Class base/sub: 0680 (bridge device)
00:00:02.954528         Command: 0001, Status: 0280
00:00:02.954529         Bus master: No
00:00:02.954530 00:0b.0 usb-ehci: 8086-265c PIIX3 IRQ9 (INTA#->IRQ19)
00:00:02.954532         Class base/sub: 0c03 (serial bus controllers)
00:00:02.954533         MMIO32 region #0: f0805000..f0805fff
00:00:02.954535         Command: 0002, Status: 0010
00:00:02.954536         Bus master: No
00:00:02.954537 !!
00:00:02.954538 !! {pciirq}
00:00:02.954538 !!
00:00:02.954539 PCI I/O APIC IRQ levels:
00:00:02.954539   IRQ16: 0
00:00:02.954540   IRQ17: 0
00:00:02.954540   IRQ18: 0
00:00:02.954541   IRQ19: 0
00:00:02.954542   IRQ20: 0
00:00:02.954542   IRQ21: 0
00:00:02.954543   IRQ22: 0
00:00:02.954543   IRQ23: 0
00:00:02.954544 !!
00:00:02.954545 !! {pcnet0}
00:00:02.954545 !!
00:00:02.954546 pcnet #0: port=d020 mmio=f0000000 mac-cfg=08:00:27:62:22:16 AM79C973 R0
00:00:02.954552 CSR0=0x0004: INIT=0 STRT=0 STOP=1 TDMD=0 TXON=0 RXON=0 IENA=0 INTR=0 IDON=0 TINT=0 RINT=0 MERR=0
00:00:02.954553               MISS=0 CERR=0 BABL=0 ERR=0
00:00:02.954555 CSR1=0x0000:
00:00:02.954556 CSR2=0x0000:
00:00:02.954556 CSR3=0x0000: BSWP=0 EMBA=0 DXMT2PD=0 LAPPEN=0 DXSUFLO=0 IDONM=0 TINTM=0 RINTM=0 MERRM=0 MISSM=0 BABLM=0
00:00:02.954559 CSR4=0x0115: JABM=1 JAB=0 TXSTRM=1 TXSTRT=0 RCVCOOM=1 RCVCCO=0 UINT=0 UINTCMD=0
00:00:02.954560               MFCOM=1 MFCO=0 ASTRP_RCV=0 APAD_XMT=0 DPOLL=0 TIMER=0 EMAPLUS=0 EN124=0
00:00:02.954562 CSR5=0x0000:
00:00:02.954563 CSR6=0x0000: RLEN=0x0* TLEN=0x0* [* encoded]
00:00:02.954564 CSR8..11=0x0000,0x0000,0x0000,0x0000: LADRF=0x0000000000000000
00:00:02.954565 CSR12..14=0x0008,0x6227,0x1622: PADR=08:00:27:62:22:16 (Current MAC Address)
00:00:02.954568 CSR15=0x0000: DXR=0 DTX=0 LOOP=0 DXMTFCS=0 FCOLL=0 DRTY=0 INTL=0 PORTSEL=0 LTR=0
00:00:02.954569               MENDECL=0 DAPC=0 DLNKTST=0 DRCVPV=0 DRCVBC=0 PROM=0
00:00:02.954571 CSR46=0x0000: POLL=0x0000 (Poll Time Counter)
00:00:02.954572 CSR47=0x0000: POLLINT=0x0000 (Poll Time Interval)
00:00:02.954573 CSR58=0x0200: SWSTYLE=0 C-LANCE / PCnet-ISA SSIZE32=0 CSRPCNET=1 APERRENT=0
00:00:02.954574 CSR112=0000: MFC=0000 (Missed receive Frame Count)
00:00:02.954576 CSR122=0000: RCVALGN=0000 (Receive Frame Align)
00:00:02.954577 CSR124=0000: RPA=0000 (Runt Packet Accept)
00:00:02.954578 BCR18=0x9001: ROMTMG=9 MEMCMD=0 EXTREQ=0
00:00:02.954578               DWIO=0 BREADE=0 BWRITE=0
00:00:02.954580 BCR32=0x0020: MIIILP=0 XPHYSP=0 XPHYFD=0 XPHYANE=1 XPHYRST=0
00:00:02.954581               DANAS=0 APDW=0 APEP=0 FMDC=0 MIIPD=0 ANTST=0
00:00:02.954583 RCVRL=0001 RCVRC=0001  GCRDRA=0
00:00:02.954584 CRDA=00000000 CRBA=00000000 CRBC=000 CRST=0000
00:00:02.954585 NRDA=00000000 NRBA=00000000 NRBC=000 NRST=0000
00:00:02.954586 NNRDA=00000000
00:00:02.954588 XMTRL=0001 XMTRC=0001  GCTDRA=00000000 BADX=00000000
00:00:02.954589 PXDA=00000000               PXBC=000 PXST=0000
00:00:02.954590 CXDA=00000000 CXBA=00000000 CXBC=000 CXST=0000
00:00:02.954590 NXDA=00000000 NXBA=00000000 NXBC=000 NXST=0000
00:00:02.954591 NNXDA=00000000
00:00:02.954595 !!
00:00:02.954595 !! {pdmtracingids}
00:00:02.954596 !!
00:00:02.954596 Device tracing IDs:
00:00:02.954597 00001  pcarch
00:00:02.954598 00002  pcbios
00:00:02.954599 00003  pci
00:00:02.954600 00004  pckbd
00:00:02.954601 00005  apic
00:00:02.954602 00006  i8259
00:00:02.954602 00007  ioapic
00:00:02.954603 00008  i8254
00:00:02.954604 00009  mc146818
00:00:02.954605 00010  8237A
00:00:02.954606 00011  VMMDev
00:00:02.954607 00012  vga
00:00:02.954608 00013  piix3ide
00:00:02.954609 00014  pcnet
00:00:02.954609 00015  ichac97
00:00:02.954610 00016  usb-ehci
00:00:02.954611 00017  usb-ohci
00:00:02.954611 00018  acpi
00:00:02.954612 USB device tracing IDs:
00:00:02.954613 Driver tracing IDs:
00:00:02.954614 01025  KeyboardQueue (level 0, lun 0, dev pckbd)
00:00:02.954615 01026  MainKeyboard (level 1, lun 0, dev pckbd)
00:00:02.954617 01027  MouseQueue (level 0, lun 1, dev pckbd)
00:00:02.954618 01028  MainMouse (level 1, lun 1, dev pckbd)
00:00:02.954619 01029  HGCM (level 0, lun 0, dev VMMDev)
00:00:02.954621 01030  MainStatus (level 0, lun 999, dev VMMDev)
00:00:02.954622 01031  MainDisplay (level 0, lun 0, dev vga)
00:00:02.954623 01032  MainStatus (level 0, lun 999, dev vga)
00:00:02.954625 01033  MainStatus (level 0, lun 999, dev piix3ide)
00:00:02.954626 01034  VD (level 0, lun 0, dev piix3ide)
00:00:02.954627 01035  VD (level 0, lun 2, dev piix3ide)
00:00:02.954628 01036  MainStatus (level 0, lun 999, dev pcnet)
00:00:02.954630 01037  NAT (level 0, lun 0, dev pcnet)
00:00:02.954631 01038  AUDIO (level 0, lun 0, dev ichac97)
00:00:02.954632 01039  ALSAAudio (level 1, lun 0, dev ichac97)
00:00:02.954634 01040  AUDIO (level 0, lun 1, dev ichac97)
00:00:02.954635 01041  AUDIO (level 0, lun 2, dev ichac97)
00:00:02.954636 01042  VUSBRootHub (level 0, lun 0, dev usb-ehci)
00:00:02.954637 01043  MainStatus (level 0, lun 999, dev usb-ehci)
00:00:02.954639 01044  VUSBRootHub (level 0, lun 0, dev usb-ohci)
00:00:02.954640 01045  MainStatus (level 0, lun 999, dev usb-ohci)
00:00:02.954641 01046  ACPIHost (level 0, lun 0, dev acpi)
00:00:02.954643 !!
00:00:02.954643 !! {pgmpoolpages}
00:00:02.954643 !!
00:00:02.954649 #01f1: HCPhys=000000007f9a4000 GCPhys=00000000dfa00000 A20  EPT_PT_FOR_PHYS  cached
00:00:02.954651 #01f2: HCPhys=000000007f9a5000 GCPhys=00000000df800000 A20  EPT_PT_FOR_PHYS  cached
00:00:02.954654 #01f3: HCPhys=000000007f9a6000 GCPhys=0000000000200000 A20  EPT_PT_FOR_PHYS  cached
00:00:02.954656 #01f4: HCPhys=000000007f9a7000 GCPhys=00000000dfc00000 A20  EPT_PT_FOR_PHYS  cached
00:00:02.954657 #01f5: HCPhys=000000007f9a8000 GCPhys=00000000dfe00000 A20  EPT_PT_FOR_PHYS  cached
00:00:02.954659 #01f6: HCPhys=000000007f9a9000 GCPhys=0000000000000000 !A20 EPT_PT_FOR_PHYS  cached
00:00:02.954661 #01f7: HCPhys=000000007f9aa000 GCPhys=0000000000000000 !A20 EPT_PD_FOR_PHYS  cached
00:00:02.954663 #01f8: HCPhys=000000007f9ab000 GCPhys=0000000000000000 !A20 EPT_PDPT_FOR_PHYS  cached
00:00:02.954665 #01f9: HCPhys=000000007f9ac000 GCPhys=8000000000000000 !A20 ROOT_NESTED  cached
00:00:02.954667 #01fa: HCPhys=000000007f9ad000 GCPhys=00000000fee00000 A20  EPT_PT_FOR_PHYS  cached
00:00:02.954669 #01fb: HCPhys=000000007f9ae000 GCPhys=0000000000000000 A20  EPT_PT_FOR_PHYS  cached
00:00:02.954671 #01fc: HCPhys=000000007f9af000 GCPhys=0000000000000000 A20  EPT_PD_FOR_PHYS  cached
00:00:02.954672 #01fd: HCPhys=000000007f9b0000 GCPhys=00000000ffe00000 A20  EPT_PT_FOR_PHYS  cached
00:00:02.954675 #01fe: HCPhys=000000007f9b1000 GCPhys=00000000c0000000 A20  EPT_PD_FOR_PHYS  cached
00:00:02.954676 #01ff: HCPhys=000000007f9b2000 GCPhys=0000000000000000 A20  EPT_PDPT_FOR_PHYS  cached
00:00:02.954678 #0200: HCPhys=000000007f9b3000 GCPhys=8000000000000000 A20  ROOT_NESTED  cached
00:00:02.954681 !!
00:00:02.954681 !! {pgmpoolroots}
00:00:02.954681 !!
00:00:02.954683 #01f9: HCPhys=000000007f9ac000 GCPhys=8000000000000000 !A20 ROOT_NESTED
00:00:02.954685 #0200: HCPhys=000000007f9b3000 GCPhys=8000000000000000 A20  ROOT_NESTED
00:00:02.954688 !!
00:00:02.954689 !! {phys}
00:00:02.954689 !!
00:00:02.954690 RAM ranges (pVM=00007fd542fc9000)
00:00:02.954691 GC Phys Range                     pbR3
00:00:02.954693 0000000000000000-000000000009ffff 0000000000000000 Conventional RAM
00:00:02.954695 00000000000a0000-00000000000bffff 0000000000000000 VGA - VGA Video Buffer
00:00:02.954697 00000000000c0000-00000000000c8fff 0000000000000000 VGA BIOS
00:00:02.954699 00000000000e0000-00000000000e0fff 0000000000000000 ACPI RSDP
00:00:02.954701 00000000000e1000-00000000000e1fff 0000000000000000 DMI tables
00:00:02.954703 00000000000e2000-00000000000effff 0000000000000000 Net Boot ROM
00:00:02.954705 00000000000f0000-00000000000fffff 0000000000000000 PC BIOS - 0xfffff
00:00:02.954707 0000000000100000-00000000001fffff 0000000000000000 Extended RAM, 1-2MB
00:00:02.954709 0000000000200000-00000000dfffffff 0000000000000000 Extended RAM, >2MB
00:00:02.954711 00000000e0000000-00000000e7ffffff 00007fd510000000 VRam
00:00:02.954713 00000000f0000000-00000000f0000fff 0000000000000000 PCnet
00:00:02.954715 00000000f0400000-00000000f07fffff 00007fd540400000 VMMDev
00:00:02.954718 00000000f0800000-00000000f0803fff 00007fd550fd3000 VMMDev Heap
00:00:02.954735 00000000f0804000-00000000f0804fff 0000000000000000 USB OHCI
00:00:02.954737 00000000f0805000-00000000f0805fff 0000000000000000 USB EHCI
00:00:02.954739 00000000fec00000-00000000fec00fff 0000000000000000 I/O APIC
00:00:02.954741 00000000fee00000-00000000fee00fff 0000000000000000 APIC
00:00:02.954743 00000000ffff0000-00000000ffffffff 0000000000000000 PC BIOS - 0xffffffff
00:00:02.954745 0000000100000000-000000011fffffff 0000000000000000 Above 4GB Base RAM
00:00:02.954747 !!
00:00:02.954747 !! {pic}
00:00:02.954748 !!
00:00:02.954748 PIC0:
00:00:02.954749  IMR :b8 ISR   :00 IRR   :00 LIRR:01
00:00:02.954751  Base:08 PriAdd:00 RegSel:00
00:00:02.954752  Poll:00 SpMask:00 IState:00
00:00:02.954752  AEOI:00 Rotate:00 FNest :00 Ini4:01
00:00:02.954753  ELCR:00 ELMask:f8
00:00:02.954754 PIC1:
00:00:02.954755  IMR :8f ISR   :00 IRR   :00 LIRR:00
00:00:02.954756  Base:70 PriAdd:00 RegSel:00
00:00:02.954756  Poll:00 SpMask:00 IState:00
00:00:02.954757  AEOI:00 Rotate:00 FNest :00 Ini4:01
00:00:02.954758  ELCR:0e ELMask:de
00:00:02.954759 !!
00:00:02.954759 !! {pirq}
00:00:02.954760 !!
00:00:02.954760 PCI IRQ levels:
00:00:02.954761   IRQA: 0
00:00:02.954761   IRQB: 0
00:00:02.954762   IRQC: 0
00:00:02.954763   IRQD: 0
00:00:02.954764 !!
00:00:02.954764 !! {pit}
00:00:02.954764 !!
00:00:02.954765 PIT (i8254) channel 0 status: irq=0x0
00:00:02.954779       count=00010000  latched_count=0000  count_latched=00
00:00:02.954779            status=00   status_latched=00     read_state=03
00:00:02.954780       write_state=03      write_latch=00        rw_mode=03
00:00:02.954781              mode=02              bcd=00           gate=01
00:00:02.954781   count_load_time=000000009e7063d1 next_transition_time=00000000a842acdd
00:00:02.954782       u64ReloadTS=00000000a4fc9484            u64NextTS=00000000a842acdd
00:00:02.954786 PIT (i8254) channel 1 status: irq=0x0
00:00:02.954787       count=00010000  latched_count=0000  count_latched=00
00:00:02.954787            status=00   status_latched=00     read_state=00
00:00:02.954788       write_state=00      write_latch=00        rw_mode=00
00:00:02.954788              mode=03              bcd=00           gate=01
00:00:02.954789   count_load_time=0000000000000000 next_transition_time=0000000000000000
00:00:02.954790       u64ReloadTS=0000000000000000            u64NextTS=ffffffffffffffff
00:00:02.954793 PIT (i8254) channel 2 status: irq=0x0
00:00:02.954793       count=0000ffff  latched_count=0000  count_latched=00
00:00:02.954794            status=00   status_latched=00     read_state=03
00:00:02.954795       write_state=03      write_latch=ff        rw_mode=03
00:00:02.954795              mode=00              bcd=00           gate=00
00:00:02.954796   count_load_time=000000009f3cd96b next_transition_time=0000000000000000
00:00:02.954796       u64ReloadTS=000000009f3cd96b            u64NextTS=ffffffffffffffff
00:00:02.954800 speaker_data_on=0x0
00:00:02.954801 !!
00:00:02.954801 !! {plugins}
00:00:02.954802 !!
00:00:02.954803 No plug-ins loaded
00:00:02.954803 !!
00:00:02.954804 !! {ps2c}
00:00:02.954804 !!
00:00:02.954805 Keyboard controller: Active command 00, DBB out FA, translation on
00:00:02.954806 Mode: 45 ( KBD_INT SYS KCC  )
00:00:02.954807 Status: 1C ( SELFTEST CMD UNLOCKED  )
00:00:02.954809 !!
00:00:02.954809 !! {ps2k}
00:00:02.954810 !!
00:00:02.954810 PS/2 Keyboard: scan set 2, scanning enabled, serial line enabled
00:00:02.954811 Active command 00
00:00:02.954812 LED state 00, Num Lock off
00:00:02.954813 Typematic delay 500ms, repeat period 91ms
00:00:02.954814 Command queue: 0 items (4 max)
00:00:02.954814 Input queue  : 0 items (64 max)
00:00:02.954816 !!
00:00:02.954816 !! {ps2m}
00:00:02.954816 !!
00:00:02.954817 PS/2 mouse state: normal, stream mode, reporting disabled, serial line enabled
00:00:02.954818 Protocol: PS/2, scaling 1:1
00:00:02.954819 Active command 00
00:00:02.954820 Sampling rate 100 reports/sec, resolution 4 counts/mm
00:00:02.954821 Command queue: 0 items (8 max)
00:00:02.954822 Event queue  : 0 items (256 max)
00:00:02.954823 !!
00:00:02.954823 !! {rtc}
00:00:02.954823 !!
00:00:02.954824 Time: 21:51:04  Date: 26-07-07
00:00:02.954826 REG A=26 B=02 C=00 D=80
00:00:02.954827 !!
00:00:02.954828 !! {svmvmcbcache}
00:00:02.954828 !!
00:00:02.954830 HM SVM is not enabled for this VM!
00:00:02.954832 HM SVM is not enabled for this VM!
00:00:02.954841 HM SVM is not enabled for this VM!
00:00:02.954848 HM SVM is not enabled for this VM!
00:00:02.954861 !!
00:00:02.954861 !! {tasks}
00:00:02.954862 !!
00:00:02.954863 Task set #0 - handle base 0, pending 0x0 RZ-enabled, running 255, 1 of 64 allocated:
00:00:02.954865  Hnd:   State     Type   pfnCallback      pvUser           Flags  Name
00:00:02.954866    0:   idle     device  00007fd540aab340 0000000000000000 0x0003 PCnet-Xmit
00:00:02.954869 !!
00:00:02.954869 !! {tracebuf}
00:00:02.954870 !!
00:00:02.954870 Tracing is disabled
00:00:02.954871 !!
00:00:02.954871 !! {trpmevent}
00:00:02.954871 !!
00:00:02.954873 CPU[0]: TRPM event (None)
00:00:02.954875 CPU[1]: TRPM event (None)
00:00:02.954881 CPU[2]: TRPM event (None)
00:00:02.954887 CPU[3]: TRPM event (None)
00:00:02.954894 !!
00:00:02.954894 !! {vbe}
00:00:02.954894 !!
00:00:02.954895 LFB at 00000000e0000000
00:00:02.954896 VBE index register: 0x000b
00:00:02.954897 VBE state (chip ID 0xb0c4):
00:00:02.954897  Display resolution: 1024 x 768 @ 32bpp
00:00:02.954899  Virtual resolution: 1024 x 32768
00:00:02.954899  Display start addr: 0, 0
00:00:02.954900  Linear scanline pitch: 0x1000
00:00:02.954900  Linear display start : 0x0000
00:00:02.954901  Selected bank: 0x0000
00:00:02.954901  DAC: 6-bit
00:00:02.954902 !!
00:00:02.954902 !! {vga}
00:00:02.954902 !!
00:00:02.954903 decoding memory at A000-AFFF
00:00:02.954904 Misc status reg. MSR:67
00:00:02.954904 pixel clock: 28.322 MHz
00:00:02.954904 double scanning off
00:00:02.954905 double clocking off
00:00:02.954905 htotal: 800 px (100 cclk)
00:00:02.954906 vtotal: 449 px
00:00:02.954906 hdisp : 1024 px (128 cclk)
00:00:02.954907 vdisp : 768 px
00:00:02.954907 split : 1023 ln
00:00:02.954908 start : 0x0
00:00:02.954909 display refresh interval: 20 ms
00:00:02.954909 !!
00:00:02.954910 !! {vgaar}
00:00:02.954910 !!
00:00:02.954910 VGA Attribute Controller (3C0): index reg 20, flip-flop: 0 (index)
00:00:02.954911  Palette: 00 01 02 03 04 05 14 07 38 39 3A 3B 3C 3D 3E 3F
00:00:02.954914  AR10:4D AR11:00 AR12:0F AR13:08 AR14:00
00:00:02.954916 !!
00:00:02.954917 !! {vgacr}
00:00:02.954917 !!
00:00:02.954917 VGA CRTC (3D5): CRTC index 3D4:14
00:00:02.954918  CR00:5F CR01:7F CR02:50 CR03:82 CR04:55 CR05:81 CR06:BF CR07:5D CR08:00 CR09:40
00:00:02.954921  CR0A:0D CR0B:0E CR0C:00 CR0D:00 CR0E:01 CR0F:40 CR10:9C CR11:00 CR12:FF CR13:80
00:00:02.954924  CR14:5F CR15:96 CR16:B9 CR17:A3 CR18:FF
00:00:02.954925 !!
00:00:02.954926 !! {vgadac}
00:00:02.954926 !!
00:00:02.954926 VGA DAC contents:
00:00:02.954927  00: 00 00 00
00:00:02.954927  01: 00 00 2A
00:00:02.954928  02: 00 2A 00
00:00:02.954929  03: 00 2A 2A
00:00:02.954929  04: 2A 00 00
00:00:02.954930  05: 2A 00 2A
00:00:02.954931  06: 2A 2A 00
00:00:02.954931  07: 2A 2A 2A
00:00:02.954932  08: 00 00 15
00:00:02.954933  09: 00 00 3F
00:00:02.954933  0A: 00 2A 15
00:00:02.954934  0B: 00 2A 3F
00:00:02.954935  0C: 2A 00 15
00:00:02.954935  0D: 2A 00 3F
00:00:02.954936  0E: 2A 2A 15
00:00:02.954937  0F: 2A 2A 3F
00:00:02.954937  10: 00 15 00
00:00:02.954938  11: 00 15 2A
00:00:02.954939  12: 00 3F 00
00:00:02.954939  13: 00 3F 2A
00:00:02.954940  14: 2A 15 00
00:00:02.954940  15: 2A 15 2A
00:00:02.954941  16: 2A 3F 00
00:00:02.954942  17: 2A 3F 2A
00:00:02.954942  18: 00 15 15
00:00:02.954943  19: 00 15 3F
00:00:02.954944  1A: 00 3F 15
00:00:02.954944  1B: 00 3F 3F
00:00:02.954945  1C: 2A 15 15
00:00:02.954945  1D: 2A 15 3F
00:00:02.954946  1E: 2A 3F 15
00:00:02.954947  1F: 2A 3F 3F
00:00:02.954947  20: 15 00 00
00:00:02.954948  21: 15 00 2A
00:00:02.954949  22: 15 2A 00
00:00:02.954949  23: 15 2A 2A
00:00:02.954950  24: 3F 00 00
00:00:02.954951  25: 3F 00 2A
00:00:02.954951  26: 3F 2A 00
00:00:02.954952  27: 3F 2A 2A
00:00:02.954952  28: 15 00 15
00:00:02.954953  29: 15 00 3F
00:00:02.954954  2A: 15 2A 15
00:00:02.954954  2B: 15 2A 3F
00:00:02.954955  2C: 3F 00 15
00:00:02.954956  2D: 3F 00 3F
00:00:02.954956  2E: 3F 2A 15
00:00:02.954957  2F: 3F 2A 3F
00:00:02.954958  30: 15 15 00
00:00:02.954958  31: 15 15 2A
00:00:02.954959  32: 15 3F 00
00:00:02.954959  33: 15 3F 2A
00:00:02.954960  34: 3F 15 00
00:00:02.954961  35: 3F 15 2A
00:00:02.954961  36: 3F 3F 00
00:00:02.954962  37: 3F 3F 2A
00:00:02.954963  38: 15 15 15
00:00:02.954963  39: 15 15 3F
00:00:02.954964  3A: 15 3F 15
00:00:02.954964  3B: 15 3F 3F
00:00:02.954965  3C: 3F 15 15
00:00:02.954966  3D: 3F 15 3F
00:00:02.954966  3E: 3F 3F 15
00:00:02.954967  3F: 3F 3F 3F
00:00:02.954968  40: 00 00 00
00:00:02.954968  41: 00 00 00
00:00:02.954969  42: 00 00 00
00:00:02.954970  43: 00 00 00
00:00:02.954970  44: 00 00 00
00:00:02.954971  45: 00 00 00
00:00:02.954971  46: 00 00 00
00:00:02.954972  47: 00 00 00
00:00:02.954973  48: 00 00 00
00:00:02.954973  49: 00 00 00
00:00:02.954974  4A: 00 00 00
00:00:02.954974  4B: 00 00 00
00:00:02.954975  4C: 00 00 00
00:00:02.954976  4D: 00 00 00
00:00:02.954976  4E: 00 00 00
00:00:02.954977  4F: 00 00 00
00:00:02.954978  50: 00 00 00
00:00:02.954978  51: 00 00 00
00:00:02.954979  52: 00 00 00
00:00:02.954979  53: 00 00 00
00:00:02.954980  54: 00 00 00
00:00:02.954981  55: 00 00 00
00:00:02.954981  56: 00 00 00
00:00:02.954982  57: 00 00 00
00:00:02.954982  58: 00 00 00
00:00:02.954983  59: 00 00 00
00:00:02.954984  5A: 00 00 00
00:00:02.954984  5B: 00 00 00
00:00:02.954985  5C: 00 00 00
00:00:02.954986  5D: 00 00 00
00:00:02.954986  5E: 00 00 00
00:00:02.954987  5F: 00 00 00
00:00:02.954987  60: 00 00 00
00:00:02.954988  61: 00 00 00
00:00:02.954989  62: 00 00 00
00:00:02.954989  63: 00 00 00
00:00:02.954990  64: 00 00 00
00:00:02.954991  65: 00 00 00
00:00:02.954991  66: 00 00 00
00:00:02.954992  67: 00 00 00
00:00:02.954992  68: 00 00 00
00:00:02.954993  69: 00 00 00
00:00:02.954994  6A: 00 00 00
00:00:02.954994  6B: 00 00 00
00:00:02.954995  6C: 00 00 00
00:00:02.954995  6D: 00 00 00
00:00:02.954996  6E: 00 00 00
00:00:02.954997  6F: 00 00 00
00:00:02.954997  70: 00 00 00
00:00:02.954998  71: 00 00 00
00:00:02.954998  72: 00 00 00
00:00:02.954999  73: 00 00 00
00:00:02.955000  74: 00 00 00
00:00:02.955000  75: 00 00 00
00:00:02.955001  76: 00 00 00
00:00:02.955001  77: 00 00 00
00:00:02.955002  78: 00 00 00
00:00:02.955003  79: 00 00 00
00:00:02.955003  7A: 00 00 00
00:00:02.955004  7B: 00 00 00
00:00:02.955005  7C: 00 00 00
00:00:02.955005  7D: 00 00 00
00:00:02.955006  7E: 00 00 00
00:00:02.955006  7F: 00 00 00
00:00:02.955007  80: 00 00 00
00:00:02.955008  81: 00 00 00
00:00:02.955008  82: 00 00 00
00:00:02.955009  83: 00 00 00
00:00:02.955009  84: 00 00 00
00:00:02.955010  85: 00 00 00
00:00:02.955011  86: 00 00 00
00:00:02.955011  87: 00 00 00
00:00:02.955012  88: 00 00 00
00:00:02.955013  89: 00 00 00
00:00:02.955013  8A: 00 00 00
00:00:02.955014  8B: 00 00 00
00:00:02.955014  8C: 00 00 00
00:00:02.955015  8D: 00 00 00
00:00:02.955016  8E: 00 00 00
00:00:02.955016  8F: 00 00 00
00:00:02.955017  90: 00 00 00
00:00:02.955017  91: 00 00 00
00:00:02.955018  92: 00 00 00
00:00:02.955019  93: 00 00 00
00:00:02.955019  94: 00 00 00
00:00:02.955020  95: 00 00 00
00:00:02.955020  96: 00 00 00
00:00:02.955021  97: 00 00 00
00:00:02.955022  98: 00 00 00
00:00:02.955022  99: 00 00 00
00:00:02.955023  9A: 00 00 00
00:00:02.955024  9B: 00 00 00
00:00:02.955024  9C: 00 00 00
00:00:02.955025  9D: 00 00 00
00:00:02.955025  9E: 00 00 00
00:00:02.955026  9F: 00 00 00
00:00:02.955027  A0: 00 00 00
00:00:02.955027  A1: 00 00 00
00:00:02.955028  A2: 00 00 00
00:00:02.955028  A3: 00 00 00
00:00:02.955029  A4: 00 00 00
00:00:02.955030  A5: 00 00 00
00:00:02.955030  A6: 00 00 00
00:00:02.955031  A7: 00 00 00
00:00:02.955032  A8: 00 00 00
00:00:02.955032  A9: 00 00 00
00:00:02.955033  AA: 00 00 00
00:00:02.955034  AB: 00 00 00
00:00:02.955034  AC: 00 00 00
00:00:02.955035  AD: 00 00 00
00:00:02.955035  AE: 00 00 00
00:00:02.955036  AF: 00 00 00
00:00:02.955037  B0: 00 00 00
00:00:02.955037  B1: 00 00 00
00:00:02.955038  B2: 00 00 00
00:00:02.955039  B3: 00 00 00
00:00:02.955039  B4: 00 00 00
00:00:02.955040  B5: 00 00 00
00:00:02.955040  B6: 00 00 00
00:00:02.955041  B7: 00 00 00
00:00:02.955042  B8: 00 00 00
00:00:02.955042  B9: 00 00 00
00:00:02.955043  BA: 00 00 00
00:00:02.955043  BB: 00 00 00
00:00:02.955044  BC: 00 00 00
00:00:02.955045  BD: 00 00 00
00:00:02.955045  BE: 00 00 00
00:00:02.955046  BF: 00 00 00
00:00:02.955047  C0: 00 00 00
00:00:02.955047  C1: 00 00 00
00:00:02.955048  C2: 00 00 00
00:00:02.955048  C3: 00 00 00
00:00:02.955049  C4: 00 00 00
00:00:02.955050  C5: 00 00 00
00:00:02.955050  C6: 00 00 00
00:00:02.955051  C7: 00 00 00
00:00:02.955051  C8: 00 00 00
00:00:02.955052  C9: 00 00 00
00:00:02.955053  CA: 00 00 00
00:00:02.955053  CB: 00 00 00
00:00:02.955054  CC: 00 00 00
00:00:02.955055  CD: 00 00 00
00:00:02.955055  CE: 00 00 00
00:00:02.955056  CF: 00 00 00
00:00:02.955057  D0: 00 00 00
00:00:02.955057  D1: 00 00 00
00:00:02.955058  D2: 00 00 00
00:00:02.955058  D3: 00 00 00
00:00:02.955059  D4: 00 00 00
00:00:02.955060  D5: 00 00 00
00:00:02.955060  D6: 00 00 00
00:00:02.955061  D7: 00 00 00
00:00:02.955061  D8: 00 00 00
00:00:02.955062  D9: 00 00 00
00:00:02.955063  DA: 00 00 00
00:00:02.955063  DB: 00 00 00
00:00:02.955064  DC: 00 00 00
00:00:02.955064  DD: 00 00 00
00:00:02.955065  DE: 00 00 00
00:00:02.955066  DF: 00 00 00
00:00:02.955066  E0: 00 00 00
00:00:02.955067  E1: 00 00 00
00:00:02.955068  E2: 00 00 00
00:00:02.955068  E3: 00 00 00
00:00:02.955069  E4: 00 00 00
00:00:02.955069  E5: 00 00 00
00:00:02.955070  E6: 00 00 00
00:00:02.955071  E7: 00 00 00
00:00:02.955071  E8: 00 00 00
00:00:02.955072  E9: 00 00 00
00:00:02.955072  EA: 00 00 00
00:00:02.955073  EB: 00 00 00
00:00:02.955074  EC: 00 00 00
00:00:02.955074  ED: 00 00 00
00:00:02.955075  EE: 00 00 00
00:00:02.955075  EF: 00 00 00
00:00:02.955076  F0: 00 00 00
00:00:02.955077  F1: 00 00 00
00:00:02.955077  F2: 00 00 00
00:00:02.955078  F3: 00 00 00
00:00:02.955079  F4: 00 00 00
00:00:02.955079  F5: 00 00 00
00:00:02.955080  F6: 00 00 00
00:00:02.955080  F7: 00 00 00
00:00:02.955081  F8: 00 00 00
00:00:02.955082  F9: 00 00 00
00:00:02.955082  FA: 00 00 00
00:00:02.955083  FB: 00 00 00
00:00:02.955083  FC: 00 00 00
00:00:02.955084  FD: 00 00 00
00:00:02.955085  FE: 00 00 00
00:00:02.955085  FF: 00 00 00
00:00:02.955086 !!
00:00:02.955086 !! {vgagr}
00:00:02.955087 !!
00:00:02.955087 VGA Graphics Controller (3CF): GR index 3CE:05
00:00:02.955087  GR00:00 GR01:00 GR02:00 GR03:00 GR04:00 GR05:50 GR06:05 GR07:0F GR08:FF
00:00:02.955090 !!
00:00:02.955090 !! {vgapl}
00:00:02.955090 !!
00:00:02.955091 read mode     : 0     write mode: 0
00:00:02.955092 set/reset data: 00    S/R enable: 00
00:00:02.955092 color compare : 00    read map  : 0
00:00:02.955093 rotate        : 0     function  : 0
00:00:02.955093 don't care    : 0F    bit mask  : FF
00:00:02.955094 seq plane mask: 0F    chain-4   : on
00:00:02.955095 !!
00:00:02.955095 !! {vgasr}
00:00:02.955095 !!
00:00:02.955095 VGA Sequencer (3C5): SR index 3C4:04
00:00:02.955096  SR00:03 SR01:00 SR02:0F SR03:00 SR04:0A SR05:00 SR06:00 SR07:01
00:00:02.955098 !!
00:00:02.955098 !! {vgatext}
00:00:02.955099 !!
00:00:02.955099 Not in text mode!
00:00:02.955100 !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
00:00:00.022542 VirtualBox VM 7.2.12 r174389 linux.amd64 (Jul  1 2026 04:31:00) release log
00:00:00.022545 Log opened 2026-07-07T18:39:06.298535000Z
00:00:00.022545 Build Type: release
00:00:00.022547 OS Product: Linux
00:00:00.022549 OS Release: 7.0.11-1-cachyos
00:00:00.022550 OS Version: #1 SMP PREEMPT_DYNAMIC Fri, 05 Jun 2026 16:36:35 +0000
00:00:00.022572 DMI Product Name: To be filled by O.E.M.
00:00:00.022582 DMI Product Version: To be filled by O.E.M.
00:00:00.022587 Firmware type: UEFI
00:00:00.022806 Secure Boot: Disabled
00:00:00.022843 Host RAM: 15948MB (15.5GB) total, 11117MB (10.8GB) available
00:00:00.022846 Executable: /usr/lib/virtualbox/VBoxHeadless
00:00:00.022846 Process ID: 6089
00:00:00.022847 Package type: LINUX_64BITS_GENERIC (OSE)
00:00:00.026771 Installed Extension Packs:
00:00:00.026778   None installed!
00:00:00.027633 Console: Machine state changed to 'Starting'
00:00:00.032715 SUP: seg #0: R   0x00000000 LB 0x0004c000
00:00:00.032738 SUP: seg #1: R X 0x0004c000 LB 0x00260000
00:00:00.032741 SUP: seg #2: R   0x002ac000 LB 0x00075000
00:00:00.032744 SUP: seg #3: RW  0x00321000 LB 0x0002db48
00:00:00.035138 SUP: Loaded VMMR0.r0 (/usr/lib/virtualbox/VMMR0.r0) at 0xXXXXXXXXXXXXXXXX - ModuleInit at XXXXXXXXXXXXXXXX and ModuleTerm at XXXXXXXXXXXXXXXX
00:00:00.035165 SUP: VMMR0EntryEx located at XXXXXXXXXXXXXXXX and VMMR0EntryFast at XXXXXXXXXXXXXXXX
00:00:00.042513 Guest architecture: x86
00:00:00.042745 Guest OS type: 'Other'
00:00:00.047038 fHMForced=true - No raw-mode support in this build!
00:00:00.047063 Using execution engine 1
00:00:00.056821 File system of '/home/vitalij/VirtualBox VMs/poler-os64-minimal/poler-os64-minimal.vdi' is ext4
00:00:00.059009 File system of '/home/vitalij/Стільниця/разроботка/Нова тека/ZCodeProject/poler-os-work/poler-os64-minimal.iso' (DVD) is ext4
00:00:00.072084 Shared Clipboard: Service loaded
00:00:00.072110 Shared Clipboard: Mode: Off
00:00:00.072161 Shared Clipboard: Service running in headless mode
00:00:00.072883 Drag and drop service loaded
00:00:00.072891 Drag and drop mode: Off
00:00:00.081501 Audio: Detected default audio driver type is 'ALSAAudio'
00:00:00.091809 ************************* CFGM dump *************************
00:00:00.091811 [/] (level 0)
00:00:00.091814   CpuExecutionCap   <integer> = 0x0000000000000064 (100)
00:00:00.091817   EnablePAE         <integer> = 0x0000000000000000 (0)
00:00:00.091818   HMEnabled         <integer> = 0x0000000000000001 (1)
00:00:00.091818   MemBalloonSize    <integer> = 0x0000000000000000 (0, 0 B)
00:00:00.091820   Name              <string>  = "poler-os64-minimal" (cb=19)
00:00:00.091821   NumCPUs           <integer> = 0x0000000000000004 (4)
00:00:00.091821   PageFusionAllowed <integer> = 0x0000000000000000 (0)
00:00:00.091822   RamHoleSize       <integer> = 0x0000000020000000 (536 870 912, 512.0 MiB)
00:00:00.091823   RamSize           <integer> = 0x0000000100000000 (4 294 967 296, 4.0 GiB)
00:00:00.091825   TimerMillies      <integer> = 0x000000000000000a (10)
00:00:00.091825   UUID              <bytes>   = "6f f8 c8 df 9e a1 af 4d 9b 7c 7d bb d7 f0 9e e8" (cb=16)
00:00:00.091828
00:00:00.091828 [/CPUM/] (level 1)
00:00:00.091829   Enable64bit        <integer> = 0x0000000000000000 (0)
00:00:00.091830   GuestCpuName       <string>  = "host" (cb=5)
00:00:00.091830   NestedHWVirt       <integer> = 0x0000000000000000 (0)
00:00:00.091831   PortableCpuIdLevel <integer> = 0x0000000000000000 (0)
00:00:00.091832   SpecCtrl           <integer> = 0x0000000000000000 (0)
00:00:00.091832
00:00:00.091832 [/CPUM/IsaExts/] (level 2)
00:00:00.091833
00:00:00.091833 [/DBGC/] (level 1)
00:00:00.091834   GlobalInitScript <string>  = "/home/vitalij/.config/VirtualBox/dbgc-init" (cb=43)
00:00:00.091835   HistoryFile      <string>  = "/home/vitalij/.config/VirtualBox/dbgc-history" (cb=46)
00:00:00.091835   LocalInitScript  <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/dbgc-init" (cb=58)
00:00:00.091836
00:00:00.091836 [/DBGF/] (level 1)
00:00:00.091836   Path <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/debug/;/home/vitalij/VirtualBox VMs/poler-os64-minimal/;cache*/home/vitalij/VirtualBox VMs/poler-os64-minimal/dbgcache/;/home/vitalij/" (cb=183)
00:00:00.091837
00:00:00.091837 [/Devices/] (level 1)
00:00:00.091838
00:00:00.091838 [/Devices/3c501/] (level 2)
00:00:00.091839
00:00:00.091839 [/Devices/8237A/] (level 2)
00:00:00.091840
00:00:00.091840 [/Devices/8237A/0/] (level 3)
00:00:00.091841   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.091841
00:00:00.091841 [/Devices/VMMDev/] (level 2)
00:00:00.091842
00:00:00.091842 [/Devices/VMMDev/0/] (level 3)
00:00:00.091843   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.091844   PCIDeviceNo   <integer> = 0x0000000000000004 (4)
00:00:00.091844   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.091845   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.091845
00:00:00.091846 [/Devices/VMMDev/0/Config/] (level 4)
00:00:00.091846   GuestCoreDumpDir <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/Snapshots" (cb=58)
00:00:00.091847
00:00:00.091847 [/Devices/VMMDev/0/LUN#0/] (level 4)
00:00:00.091848   Driver <string>  = "HGCM" (cb=5)
00:00:00.091848
00:00:00.091849 [/Devices/VMMDev/0/LUN#0/Config/] (level 5)
00:00:00.091850
00:00:00.091850 [/Devices/VMMDev/0/LUN#999/] (level 4)
00:00:00.091850   Driver <string>  = "MainStatus" (cb=11)
00:00:00.091851
00:00:00.091851 [/Devices/VMMDev/0/LUN#999/Config/] (level 5)
00:00:00.091852   First                <integer> = 0x0000000000000000 (0)
00:00:00.091853   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.091854   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.091855   iLedSet              <integer> = 0x0000000000000005 (5)
00:00:00.091856
00:00:00.091857 [/Devices/acpi/] (level 2)
00:00:00.091858
00:00:00.091858 [/Devices/acpi/0/] (level 3)
00:00:00.091860   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.091861   PCIDeviceNo   <integer> = 0x0000000000000007 (7)
00:00:00.091861   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.091863   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.091863
00:00:00.091863 [/Devices/acpi/0/Config/] (level 4)
00:00:00.091864   CpuHotPlug          <integer> = 0x0000000000000000 (0)
00:00:00.091865   FdcEnabled          <integer> = 0x0000000000000000 (0)
00:00:00.091866   HostBusPciAddress   <integer> = 0x0000000000000000 (0)
00:00:00.091866   HpetEnabled         <integer> = 0x0000000000000000 (0)
00:00:00.091867   IOAPIC              <integer> = 0x0000000000000001 (1)
00:00:00.091867   IocPciAddress       <integer> = 0x0000000000010000 (65 536)
00:00:00.091868   NumCPUs             <integer> = 0x0000000000000004 (4)
00:00:00.091869   Parallel0IoPortBase <integer> = 0x0000000000000000 (0)
00:00:00.091869   Parallel0Irq        <integer> = 0x0000000000000000 (0)
00:00:00.091870   Parallel1IoPortBase <integer> = 0x0000000000000000 (0)
00:00:00.091871   Parallel1Irq        <integer> = 0x0000000000000000 (0)
00:00:00.091871   Serial0IoPortBase   <integer> = 0x0000000000000000 (0)
00:00:00.091872   Serial0Irq          <integer> = 0x0000000000000000 (0)
00:00:00.091872   Serial1IoPortBase   <integer> = 0x0000000000000000 (0)
00:00:00.091873   Serial1Irq          <integer> = 0x0000000000000000 (0)
00:00:00.091873   ShowCpu             <integer> = 0x0000000000000001 (1)
00:00:00.091874   ShowRtc             <integer> = 0x0000000000000000 (0)
00:00:00.091875   SmcEnabled          <integer> = 0x0000000000000000 (0)
00:00:00.091875
00:00:00.091876 [/Devices/acpi/0/LUN#0/] (level 4)
00:00:00.091876   Driver <string>  = "ACPIHost" (cb=9)
00:00:00.091877
00:00:00.091877 [/Devices/acpi/0/LUN#0/Config/] (level 5)
00:00:00.091878
00:00:00.091878 [/Devices/acpi/0/LUN#1/] (level 4)
00:00:00.091879   Driver <string>  = "ACPICpu" (cb=8)
00:00:00.091879
00:00:00.091879 [/Devices/acpi/0/LUN#1/Config/] (level 5)
00:00:00.091880
00:00:00.091880 [/Devices/acpi/0/LUN#2/] (level 4)
00:00:00.091881   Driver <string>  = "ACPICpu" (cb=8)
00:00:00.091881
00:00:00.091882 [/Devices/acpi/0/LUN#2/Config/] (level 5)
00:00:00.091882
00:00:00.091883 [/Devices/acpi/0/LUN#3/] (level 4)
00:00:00.091883   Driver <string>  = "ACPICpu" (cb=8)
00:00:00.091884
00:00:00.091884 [/Devices/acpi/0/LUN#3/Config/] (level 5)
00:00:00.091885
00:00:00.091885 [/Devices/apic/] (level 2)
00:00:00.091886
00:00:00.091886 [/Devices/apic/0/] (level 3)
00:00:00.091886   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.091887
00:00:00.091887 [/Devices/apic/0/Config/] (level 4)
00:00:00.091888   IOAPIC  <integer> = 0x0000000000000001 (1)
00:00:00.091889   Mode    <integer> = 0x0000000000000002 (2)
00:00:00.091889   NumCPUs <integer> = 0x0000000000000004 (4)
00:00:00.091890
00:00:00.091890 [/Devices/dp8390/] (level 2)
00:00:00.091891
00:00:00.091891 [/Devices/e1000/] (level 2)
00:00:00.091891
00:00:00.091892 [/Devices/i8254/] (level 2)
00:00:00.091892
00:00:00.091893 [/Devices/i8254/0/] (level 3)
00:00:00.091893
00:00:00.091893 [/Devices/i8254/0/Config/] (level 4)
00:00:00.091894
00:00:00.091894 [/Devices/i8259/] (level 2)
00:00:00.091895
00:00:00.091895 [/Devices/i8259/0/] (level 3)
00:00:00.091896   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.091896
00:00:00.091897 [/Devices/i8259/0/Config/] (level 4)
00:00:00.091898
00:00:00.091898 [/Devices/ichac97/] (level 2)
00:00:00.091899
00:00:00.091900 [/Devices/ichac97/0/] (level 3)
00:00:00.091901   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.091902   PCIDeviceNo   <integer> = 0x0000000000000005 (5)
00:00:00.091903   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.091904   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.091905
00:00:00.091905 [/Devices/ichac97/0/AudioConfig/] (level 4)
00:00:00.091907
00:00:00.091907 [/Devices/ichac97/0/Config/] (level 4)
00:00:00.091909   Codec        <string>  = "STAC9700" (cb=9)
00:00:00.091909   DebugEnabled <integer> = 0x0000000000000000 (0)
00:00:00.091910
00:00:00.091910 [/Devices/ichac97/0/LUN#0/] (level 4)
00:00:00.091911   Driver <string>  = "AUDIO" (cb=6)
00:00:00.091911
00:00:00.091912 [/Devices/ichac97/0/LUN#0/AttachedDriver/] (level 5)
00:00:00.091912   Driver <string>  = "ALSAAudio" (cb=10)
00:00:00.091913
00:00:00.091913 [/Devices/ichac97/0/LUN#0/AttachedDriver/Config/] (level 6)
00:00:00.091914
00:00:00.091914 [/Devices/ichac97/0/LUN#0/Config/] (level 5)
00:00:00.091915   DriverName    <string>  = "ALSAAudio" (cb=10)
00:00:00.091916   InputEnabled  <integer> = 0x0000000000000000 (0)
00:00:00.091916   OutputEnabled <integer> = 0x0000000000000001 (1)
00:00:00.091917
00:00:00.091917 [/Devices/ichac97/0/LUN#1/] (level 4)
00:00:00.091918   Driver <string>  = "AUDIO" (cb=6)
00:00:00.091918
00:00:00.091918 [/Devices/ichac97/0/LUN#2/] (level 4)
00:00:00.091919   Driver <string>  = "AUDIO" (cb=6)
00:00:00.091919
00:00:00.091920 [/Devices/ioapic/] (level 2)
00:00:00.091920
00:00:00.091920 [/Devices/ioapic/0/] (level 3)
00:00:00.091921   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.091922
00:00:00.091922 [/Devices/ioapic/0/Config/] (level 4)
00:00:00.091923   NumCPUs <integer> = 0x0000000000000004 (4)
00:00:00.091923
00:00:00.091923 [/Devices/mc146818/] (level 2)
00:00:00.091924
00:00:00.091924 [/Devices/mc146818/0/] (level 3)
00:00:00.091925
00:00:00.091925 [/Devices/mc146818/0/Config/] (level 4)
00:00:00.091926   UseUTC <integer> = 0x0000000000000000 (0)
00:00:00.091926
00:00:00.091927 [/Devices/parallel/] (level 2)
00:00:00.091927
00:00:00.091927 [/Devices/pcarch/] (level 2)
00:00:00.091928
00:00:00.091928 [/Devices/pcarch/0/] (level 3)
00:00:00.091929   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.091929
00:00:00.091929 [/Devices/pcarch/0/Config/] (level 4)
00:00:00.091930
00:00:00.091930 [/Devices/pcbios/] (level 2)
00:00:00.091931
00:00:00.091931 [/Devices/pcbios/0/] (level 3)
00:00:00.091932   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.091932
00:00:00.091933 [/Devices/pcbios/0/Config/] (level 4)
00:00:00.091934   APIC            <integer> = 0x0000000000000001 (1)
00:00:00.091935   BootDevice0     <string>  = "FLOPPY" (cb=7)
00:00:00.091935   BootDevice1     <string>  = "DVD" (cb=4)
00:00:00.091936   BootDevice2     <string>  = "IDE" (cb=4)
00:00:00.091936   BootDevice3     <string>  = "NONE" (cb=5)
00:00:00.091937   DmiSystemSerial <string>  = "VirtualBox-<DmiSystemUuid>" (cb=27)
00:00:00.091937   FloppyDevice    <string>  = "i82078" (cb=7)
00:00:00.091938   HardDiskDevice  <string>  = "piix3ide" (cb=9)
00:00:00.091939   IOAPIC          <integer> = 0x0000000000000001 (1)
00:00:00.091940   McfgBase        <integer> = 0x0000000000000000 (0)
00:00:00.091941   McfgLength      <integer> = 0x0000000000000000 (0)
00:00:00.091942   NumCPUs         <integer> = 0x0000000000000004 (4)
00:00:00.091943   PXEDebug        <integer> = 0x0000000000000000 (0)
00:00:00.091944   UUID            <bytes>   = "6f f8 c8 df 9e a1 af 4d 9b 7c 7d bb d7 f0 9e e8" (cb=16)
00:00:00.091947   UuidLe          <integer> = 0x0000000000000001 (1)
00:00:00.091949
00:00:00.091949 [/Devices/pcbios/0/Config/NetBoot/] (level 5)
00:00:00.091950
00:00:00.091950 [/Devices/pcbios/0/Config/NetBoot/0/] (level 6)
00:00:00.091951   NIC           <integer> = 0x0000000000000000 (0)
00:00:00.091952   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.091953   PCIDeviceNo   <integer> = 0x0000000000000003 (3)
00:00:00.091953   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.091954
00:00:00.091954 [/Devices/pci/] (level 2)
00:00:00.091955
00:00:00.091955 [/Devices/pci/0/] (level 3)
00:00:00.091956   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.091956
00:00:00.091956 [/Devices/pci/0/Config/] (level 4)
00:00:00.091957   IOAPIC <integer> = 0x0000000000000001 (1)
00:00:00.091958
00:00:00.091958 [/Devices/pcibridge/] (level 2)
00:00:00.091958
00:00:00.091959 [/Devices/pckbd/] (level 2)
00:00:00.091959
00:00:00.091959 [/Devices/pckbd/0/] (level 3)
00:00:00.091960   Trusted <integer> = 0x0000000000000001 (1)
00:00:00.091961
00:00:00.091961 [/Devices/pckbd/0/Config/] (level 4)
00:00:00.091962
00:00:00.091962 [/Devices/pckbd/0/LUN#0/] (level 4)
00:00:00.091963   Driver <string>  = "KeyboardQueue" (cb=14)
00:00:00.091963
00:00:00.091963 [/Devices/pckbd/0/LUN#0/AttachedDriver/] (level 5)
00:00:00.091964   Driver <string>  = "MainKeyboard" (cb=13)
00:00:00.091965
00:00:00.091965 [/Devices/pckbd/0/LUN#0/Config/] (level 5)
00:00:00.091966   QueueSize <integer> = 0x0000000000000040 (64, 64 B)
00:00:00.091966
00:00:00.091967 [/Devices/pckbd/0/LUN#1/] (level 4)
00:00:00.091967   Driver <string>  = "MouseQueue" (cb=11)
00:00:00.091968
00:00:00.091968 [/Devices/pckbd/0/LUN#1/AttachedDriver/] (level 5)
00:00:00.091969   Driver <string>  = "MainMouse" (cb=10)
00:00:00.091969
00:00:00.091969 [/Devices/pckbd/0/LUN#1/Config/] (level 5)
00:00:00.091970   QueueSize <integer> = 0x0000000000000080 (128, 128 B)
00:00:00.091971
00:00:00.091971 [/Devices/pcnet/] (level 2)
00:00:00.091972
00:00:00.091972 [/Devices/pcnet/0/] (level 3)
00:00:00.091973   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.091974   PCIDeviceNo   <integer> = 0x0000000000000003 (3)
00:00:00.091974   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.091975   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.091975
00:00:00.091975 [/Devices/pcnet/0/Config/] (level 4)
00:00:00.091976   CableConnected <integer> = 0x0000000000000001 (1)
00:00:00.091977   ChipType       <string>  = "Am79C973" (cb=9)
00:00:00.091977   LineSpeed      <integer> = 0x0000000000000000 (0)
00:00:00.091978   MAC            <bytes>   = "08 00 27 62 22 16" (cb=6)
00:00:00.091979
00:00:00.091979 [/Devices/pcnet/0/LUN#0/] (level 4)
00:00:00.091980   Driver <string>  = "NAT" (cb=4)
00:00:00.091980
00:00:00.091980 [/Devices/pcnet/0/LUN#0/Config/] (level 5)
00:00:00.091982   AliasMode          <integer> = 0x0000000000000000 (0)
00:00:00.091982   DNSProxy           <integer> = 0x0000000000000000 (0)
00:00:00.091983   EnableTFTP         <integer> = 0x0000000000000000 (0)
00:00:00.091983   ForwardBroadcast   <integer> = 0x0000000000000000 (0)
00:00:00.091984   LocalhostReachable <integer> = 0x0000000000000001 (1)
00:00:00.091985   Network            <string>  = "10.0.2.0/24" (cb=12)
00:00:00.091985   PassDomain         <integer> = 0x0000000000000001 (1)
00:00:00.091986   UseHostResolver    <integer> = 0x0000000000000000 (0)
00:00:00.091986
00:00:00.091986 [/Devices/pcnet/0/LUN#999/] (level 4)
00:00:00.091987   Driver <string>  = "MainStatus" (cb=11)
00:00:00.091988
00:00:00.091988 [/Devices/pcnet/0/LUN#999/Config/] (level 5)
00:00:00.091989   First                <integer> = 0x0000000000000000 (0)
00:00:00.091989   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.091990   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.091991   iLedSet              <integer> = 0x0000000000000004 (4)
00:00:00.091991
00:00:00.091991 [/Devices/piix3ide/] (level 2)
00:00:00.091992
00:00:00.091992 [/Devices/piix3ide/0/] (level 3)
00:00:00.091993   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.091994   PCIDeviceNo   <integer> = 0x0000000000000001 (1)
00:00:00.091994   PCIFunctionNo <integer> = 0x0000000000000001 (1)
00:00:00.091995   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.091995
00:00:00.091995 [/Devices/piix3ide/0/Config/] (level 4)
00:00:00.091996   Type <string>  = "PIIX4" (cb=6)
00:00:00.091997
00:00:00.091997 [/Devices/piix3ide/0/LUN#0/] (level 4)
00:00:00.091999   Driver <string>  = "VD" (cb=3)
00:00:00.092000
00:00:00.092000 [/Devices/piix3ide/0/LUN#0/Config/] (level 5)
00:00:00.092002   Format    <string>  = "VDI" (cb=4)
00:00:00.092003   Mountable <integer> = 0x0000000000000000 (0)
00:00:00.092004   Path      <string>  = "/home/vitalij/VirtualBox VMs/poler-os64-minimal/poler-os64-minimal.vdi" (cb=71)
00:00:00.092005   Type      <string>  = "HardDisk" (cb=9)
00:00:00.092006
00:00:00.092006 [/Devices/piix3ide/0/LUN#0/Config/VDConfig/] (level 6)
00:00:00.092007   AllocationBlockSize <string>  = "1048576" (cb=8)
00:00:00.092008
00:00:00.092008 [/Devices/piix3ide/0/LUN#2/] (level 4)
00:00:00.092009   Driver <string>  = "VD" (cb=3)
00:00:00.092009
00:00:00.092009 [/Devices/piix3ide/0/LUN#2/Config/] (level 5)
00:00:00.092010   Format    <string>  = "RAW" (cb=4)
00:00:00.092011   Mountable <integer> = 0x0000000000000001 (1)
00:00:00.092011   Path      <string>  = "/home/vitalij/Стільниця/разроботка/Нова тека/ZCodeProject/poler-os-work/poler-os64-minimal.iso" (cb=122)
00:00:00.092012   ReadOnly  <integer> = 0x0000000000000001 (1)
00:00:00.092012   Type      <string>  = "DVD" (cb=4)
00:00:00.092013
00:00:00.092013 [/Devices/piix3ide/0/LUN#999/] (level 4)
00:00:00.092014   Driver <string>  = "MainStatus" (cb=11)
00:00:00.092014
00:00:00.092014 [/Devices/piix3ide/0/LUN#999/Config/] (level 5)
00:00:00.092015   DeviceInstance       <string>  = "piix3ide/0" (cb=11)
00:00:00.092016   First                <integer> = 0x0000000000000000 (0)
00:00:00.092016   HasMediumAttachments <integer> = 0x0000000000000001 (1)
00:00:00.092017   Last                 <integer> = 0x0000000000000003 (3)
00:00:00.092018   iLedSet              <integer> = 0x0000000000000003 (3)
00:00:00.092018
00:00:00.092018 [/Devices/serial/] (level 2)
00:00:00.092019
00:00:00.092019 [/Devices/usb-ehci/] (level 2)
00:00:00.092020
00:00:00.092020 [/Devices/usb-ehci/0/] (level 3)
00:00:00.092021   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.092022   PCIDeviceNo   <integer> = 0x000000000000000b (11)
00:00:00.092022   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.092023   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.092023
00:00:00.092023 [/Devices/usb-ehci/0/Config/] (level 4)
00:00:00.092024
00:00:00.092024 [/Devices/usb-ehci/0/LUN#0/] (level 4)
00:00:00.092025   Driver <string>  = "VUSBRootHub" (cb=12)
00:00:00.092025
00:00:00.092026 [/Devices/usb-ehci/0/LUN#0/Config/] (level 5)
00:00:00.092027
00:00:00.092027 [/Devices/usb-ehci/0/LUN#999/] (level 4)
00:00:00.092027   Driver <string>  = "MainStatus" (cb=11)
00:00:00.092028
00:00:00.092028 [/Devices/usb-ehci/0/LUN#999/Config/] (level 5)
00:00:00.092029   First                <integer> = 0x0000000000000000 (0)
00:00:00.092030   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.092030   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.092031   iLedSet              <integer> = 0x0000000000000002 (2)
00:00:00.092031
00:00:00.092032 [/Devices/usb-ohci/] (level 2)
00:00:00.092032
00:00:00.092032 [/Devices/usb-ohci/0/] (level 3)
00:00:00.092033   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.092034   PCIDeviceNo   <integer> = 0x0000000000000006 (6)
00:00:00.092034   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.092035   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.092035
00:00:00.092035 [/Devices/usb-ohci/0/Config/] (level 4)
00:00:00.092036
00:00:00.092036 [/Devices/usb-ohci/0/LUN#0/] (level 4)
00:00:00.092037   Driver <string>  = "VUSBRootHub" (cb=12)
00:00:00.092037
00:00:00.092038 [/Devices/usb-ohci/0/LUN#0/Config/] (level 5)
00:00:00.092039
00:00:00.092039 [/Devices/usb-ohci/0/LUN#999/] (level 4)
00:00:00.092039   Driver <string>  = "MainStatus" (cb=11)
00:00:00.092040
00:00:00.092040 [/Devices/usb-ohci/0/LUN#999/Config/] (level 5)
00:00:00.092041   First                <integer> = 0x0000000000000000 (0)
00:00:00.092042   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.092043   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.092044   iLedSet              <integer> = 0x0000000000000001 (1)
00:00:00.092045
00:00:00.092046 [/Devices/vga/] (level 2)
00:00:00.092047
00:00:00.092047 [/Devices/vga/0/] (level 3)
00:00:00.092048   PCIBusNo      <integer> = 0x0000000000000000 (0)
00:00:00.092050   PCIDeviceNo   <integer> = 0x0000000000000002 (2)
00:00:00.092050   PCIFunctionNo <integer> = 0x0000000000000000 (0)
00:00:00.092051   Trusted       <integer> = 0x0000000000000001 (1)
00:00:00.092052
00:00:00.092053 [/Devices/vga/0/Config/] (level 4)
00:00:00.092054   3DEnabled        <integer> = 0x0000000000000000 (0)
00:00:00.092056   CustomVideoModes <integer> = 0x0000000000000000 (0)
00:00:00.092056   FadeIn           <integer> = 0x0000000000000001 (1)
00:00:00.092058   FadeOut          <integer> = 0x0000000000000001 (1)
00:00:00.092059   HeightReduction  <integer> = 0x0000000000000000 (0)
00:00:00.092060   LogoFile         <string>  = "" (cb=1)
00:00:00.092060   LogoTime         <integer> = 0x0000000000000000 (0)
00:00:00.092061   MonitorCount     <integer> = 0x0000000000000008 (8)
00:00:00.092062   ShowBootMenu     <integer> = 0x0000000000000002 (2)
00:00:00.092062   VRamSize         <integer> = 0x0000000008000000 (134 217 728, 128.0 MiB)
00:00:00.092063
00:00:00.092064 [/Devices/vga/0/LUN#0/] (level 4)
00:00:00.092064   Driver <string>  = "MainDisplay" (cb=12)
00:00:00.092065
00:00:00.092065 [/Devices/vga/0/LUN#0/Config/] (level 5)
00:00:00.092066
00:00:00.092066 [/Devices/vga/0/LUN#999/] (level 4)
00:00:00.092067   Driver <string>  = "MainStatus" (cb=11)
00:00:00.092067
00:00:00.092067 [/Devices/vga/0/LUN#999/Config/] (level 5)
00:00:00.092068   First                <integer> = 0x0000000000000000 (0)
00:00:00.092069   HasMediumAttachments <integer> = 0x0000000000000000 (0)
00:00:00.092069   Last                 <integer> = 0x0000000000000000 (0)
00:00:00.092070   iLedSet              <integer> = 0x0000000000000000 (0)
00:00:00.092071
00:00:00.092071 [/Devices/virtio-net/] (level 2)
00:00:00.092072
00:00:00.092072 [/EM/] (level 1)
00:00:00.092073   TripleFaultReset <integer> = 0x0000000000000000 (0)
00:00:00.092073
00:00:00.092073 [/GCM/] (level 1)
00:00:00.092074
00:00:00.092074 [/GIM/] (level 1)
00:00:00.092075   Provider <string>  = "None" (cb=5)
00:00:00.092075
00:00:00.092075 [/HM/] (level 1)
00:00:00.092076   64bitEnabled        <integer> = 0x0000000000000000 (0)
00:00:00.092077   EnableLargePages    <integer> = 0x0000000000000000 (0)
00:00:00.092077   EnableNestedPaging  <integer> = 0x0000000000000001 (1)
00:00:00.092078   EnableUX            <integer> = 0x0000000000000001 (1)
00:00:00.092079   EnableVPID          <integer> = 0x0000000000000001 (1)
00:00:00.092079   Exclusive           <integer> = 0x0000000000000001 (1)
00:00:00.092080   HMForced            <integer> = 0x0000000000000001 (1)
00:00:00.092080   IBPBOnVMEntry       <integer> = 0x0000000000000000 (0)
00:00:00.092081   IBPBOnVMExit        <integer> = 0x0000000000000000 (0)
00:00:00.092082   L1DFlushOnSched     <integer> = 0x0000000000000001 (1)
00:00:00.092082   L1DFlushOnVMEntry   <integer> = 0x0000000000000000 (0)
00:00:00.092083   MDSClearOnSched     <integer> = 0x0000000000000001 (1)
00:00:00.092083   MDSClearOnVMEntry   <integer> = 0x0000000000000000 (0)
00:00:00.092084   SpecCtrlByHost      <integer> = 0x0000000000000000 (0)
00:00:00.092084   SvmVirtVmsaveVmload <integer> = 0x0000000000000000 (0)
00:00:00.092085   UseNEMInstead       <integer> = 0x0000000000000000 (0)
00:00:00.092085
00:00:00.092086 [/MM/] (level 1)
00:00:00.092086   CanUseLargerHeap <integer> = 0x0000000000000000 (0)
00:00:00.092087
00:00:00.092087 [/NEM/] (level 1)
00:00:00.092087   Allow64BitGuests  <integer> = 0x0000000000000000 (0)
00:00:00.092088   IBPBOnVMEntry     <integer> = 0x0000000000000000 (0)
00:00:00.092088   IBPBOnVMExit      <integer> = 0x0000000000000000 (0)
00:00:00.092089   L1DFlushOnSched   <integer> = 0x0000000000000001 (1)
00:00:00.092090   L1DFlushOnVMEntry <integer> = 0x0000000000000000 (0)
00:00:00.092090   MDSClearOnSched   <integer> = 0x0000000000000001 (1)
00:00:00.092091   MDSClearOnVMEntry <integer> = 0x0000000000000000 (0)
00:00:00.092091
00:00:00.092091 [/PDM/] (level 1)
00:00:00.092092
00:00:00.092092 [/PDM/AsyncCompletion/] (level 2)
00:00:00.092093
00:00:00.092093 [/PDM/AsyncCompletion/File/] (level 3)
00:00:00.092094
00:00:00.092094 [/PDM/AsyncCompletion/File/BwGroups/] (level 4)
00:00:00.092095
00:00:00.092095 [/PDM/BlkCache/] (level 2)
00:00:00.092096   CacheSize <integer> = 0x0000000000500000 (5 242 880, 5.0 MiB)
00:00:00.092097
00:00:00.092098 [/PDM/Devices/] (level 2)
00:00:00.092099
00:00:00.092099 [/PDM/Drivers/] (level 2)
00:00:00.092100
00:00:00.092100 [/PDM/Drivers/VBoxC/] (level 3)
00:00:00.092102   Path <string>  = "/usr/lib/virtualbox/components/VBoxC" (cb=37)
00:00:00.092102
00:00:00.092103 [/PDM/NetworkShaper/] (level 2)
00:00:00.092104
00:00:00.092104 [/PDM/NetworkShaper/BwGroups/] (level 3)
00:00:00.092105
00:00:00.092106 [/TM/] (level 1)
00:00:00.092107   UTCOffset <integer> = 0x0000000000000000 (0)
00:00:00.092107
00:00:00.092108 [/USB/] (level 1)
00:00:00.092108
00:00:00.092108 [/USB/USBProxy/] (level 2)
00:00:00.092109
00:00:00.092109 [/USB/USBProxy/GlobalConfig/] (level 3)
00:00:00.092110
00:00:00.092110 ********************* End of CFGM dump **********************
00:00:00.092224 HM: HMR3Init: VT-x w/ nested paging and unrestricted guest execution hw support
00:00:00.092271 CPUM: fXStateHostMask=0x7; host XCR0=0x7
00:00:00.092513 CPUM: Matched host CPU INTEL 0x6/0x3a/0x9 Intel_Core7_IvyBridge with CPU DB entry 'Intel Core i5-3570' (INTEL 0x6/0x3a/0x9 Intel_Core7_IvyBridge)
00:00:00.092551 CPUM: MXCSR_MASK=0xffff (host: 0xffff)
00:00:00.092561 CPUM: Microcode revision 0x00000021
00:00:00.092573 CPUM: MSR/CPUID reconciliation insert: 0x0000010b IA32_FLUSH_CMD
00:00:00.092585 CPUM: Enabled MTRR read-write support
00:00:00.092588 CPUM: Enabled fixed-range MTRRs and 16 (virtualized) variable-range MTRRs
00:00:00.094127 PGM: Host paging mode: AMD64+PGE+NX
00:00:00.094136 PGM: PGMPool: cMaxPages=2304 (u64MaxPages=2084)
00:00:00.094138 PGM: pgmR3PoolInit: cMaxPages=0x900 cMaxUsers=0x1200 cMaxPhysExts=0x1200 fCacheEnable=true
00:00:00.095072 PGM: /proc/sys/vm/max_map_count = 1048576 (rc2=VWRN_TRAILING_CHARS); cGuessNeeded=16384
00:00:00.102451 TM: GIP - u32Mode=3 (Invariant) u32UpdateHz=100 u32UpdateIntervalNS=10000000 enmUseTscDelta=2 (Practically Zero) fGetGipCpu=0x1b cCpus=8
00:00:00.102472 TM: GIP - u64CpuHz=3 392 292 995 (0xca324883)  SUPGetCpuHzFromGip => 3 392 292 995
00:00:00.102478 TM: GIP - CPU: iCpuSet=0x0 idCpu=0x0 idApic=0x0 iGipCpu=0x5 i64TSCDelta=0 enmState=3 u64CpuHz=3392292982(*) cErrors=0
00:00:00.102483 TM: GIP - CPU: iCpuSet=0x1 idCpu=0x1 idApic=0x2 iGipCpu=0x7 i64TSCDelta=0 enmState=3 u64CpuHz=3392292892(*) cErrors=0
00:00:00.102495 TM: GIP - CPU: iCpuSet=0x2 idCpu=0x2 idApic=0x4 iGipCpu=0x0 i64TSCDelta=0 enmState=3 u64CpuHz=3392292995(*) cErrors=0
00:00:00.102500 TM: GIP - CPU: iCpuSet=0x3 idCpu=0x3 idApic=0x6 iGipCpu=0x3 i64TSCDelta=0 enmState=3 u64CpuHz=3392292755(*) cErrors=0
00:00:00.102503 TM: GIP - CPU: iCpuSet=0x4 idCpu=0x4 idApic=0x1 iGipCpu=0x4 i64TSCDelta=0 enmState=3 u64CpuHz=3392292741(*) cErrors=0
00:00:00.102506 TM: GIP - CPU: iCpuSet=0x5 idCpu=0x5 idApic=0x3 iGipCpu=0x6 i64TSCDelta=0 enmState=3 u64CpuHz=3392292873(*) cErrors=0
00:00:00.102509 TM: GIP - CPU: iCpuSet=0x6 idCpu=0x6 idApic=0x5 iGipCpu=0x1 i64TSCDelta=0 enmState=3 u64CpuHz=3392285794(*) cErrors=0
00:00:00.102513 TM: GIP - CPU: iCpuSet=0x7 idCpu=0x7 idApic=0x7 iGipCpu=0x2 i64TSCDelta=0 enmState=3 u64CpuHz=3392290916(*) cErrors=0
00:00:00.102526 TM:     cTSCTicksPerSecond=3 392 292 995 (0xca324883) enmTSCMode=1 (VirtTSCEmulated) TSCMultiplier=1
00:00:00.102528 TM: cTSCTicksPerSecondHost=3 392 292 995 (0xca324883)
00:00:00.102529 TM: TSCTiedToExecution=false TSCNotTiedToHalt=false
00:00:00.103248 EMR3Init: fIemExecutesAll=false fGuruOnTripleFault=true
00:00:00.103671 IEM: TargetCpu=CURRENT, Microarch=Intel_Core7_IvyBridge aidxTargetCpuEflFlavour={1,0}
00:00:00.105035 GIM: Using provider 'None' (Implementation version: 0)
00:00:00.105048 GCM: Initialized - Fixer bits: 0x0
00:00:00.117359 AIOMgr: Default manager type is 'Async'
00:00:00.117385 AIOMgr: Default file backend is 'NonBuffered'
00:00:00.117476 BlkCache: Cache successfully initialized. Cache size is 5242880 bytes
00:00:00.117480 BlkCache: Cache commit interval is 10000 ms
00:00:00.117482 BlkCache: Cache commit threshold is 2621440 bytes
00:00:00.118866 PcBios: [SMP] BIOS with 4 CPUs
00:00:00.118887 PcBios: Using the 386+ BIOS image.
00:00:00.118965 PcBios: MPS table at 000e1300
00:00:00.119631 PcBios: fCheckShutdownStatusForSoftReset=true  fClearShutdownStatusOnHardReset=true
00:00:00.120150 SUP: seg #0: R   0x00000000 LB 0x00009000
00:00:00.120156 SUP: seg #1: R X 0x00009000 LB 0x00030000
00:00:00.120159 SUP: seg #2: R   0x00039000 LB 0x0000f000
00:00:00.120162 SUP: seg #3: RW  0x00048000 LB 0x00007500
00:00:00.120278 SUP: Loaded VBoxDDR0.r0 (/usr/lib/virtualbox/VBoxDDR0.r0) at 0xXXXXXXXXXXXXXXXX - ModuleInit at XXXXXXXXXXXXXXXX and ModuleTerm at XXXXXXXXXXXXXXXX
00:00:00.120518 PDM: VirtualBox APIC backend registered
00:00:00.120527 CPUM: SetGuestCpuIdFeature: Enabled xAPIC
00:00:00.121948 IOAPIC: Version=2.0 ChipType=ICH9
00:00:00.122010 PIT: mode=3 count=0x10000 (65536) - 18.20 Hz (ch=0)
00:00:00.122224 VMMDev: cbDefaultBudget: 696 794 965 (29883f55)
00:00:00.124233 Shared Folders service loaded
00:00:00.124662 Guest Control service loaded
00:00:00.154456 VGA: Using the 386+ BIOS image.
00:00:00.155133 DrvVD: Flushes will be ignored
00:00:00.155137 DrvVD: Async flushes will be passed to the disk
00:00:00.155216 VD: VDInit finished with VINF_SUCCESS
00:00:00.155240 VD: Opening the disk took 98979 ns
00:00:00.155257 PIIX3 ATA: LUN#0: disk, PCHS=4161/16/63, total number of sectors 4194304
00:00:00.156677 PIIX3 ATA: LUN#1: no unit
00:00:00.156794 DrvVD: Flushes will be ignored
00:00:00.156797 DrvVD: Async flushes will be passed to the disk
00:00:00.156816 VD: Opening the disk took 16007 ns
00:00:00.156829 PIIX3 ATA: LUN#2: CD/DVD, total number of sectors 16327, passthrough disabled
00:00:00.156881 PIIX3 ATA: LUN#3: no unit
00:00:00.156960 PIIX3 ATA: Ctl#0: finished processing RESET
00:00:00.156977 PIIX3 ATA: Ctl#1: finished processing RESET
00:00:00.157416 AC97: Using codec 'STAC9700'
00:00:00.157477 Audio: Initializing ALSA driver
00:00:00.190358 ALSA: The ALSAAudio plugin for pulse audio is being used (pulse).
00:00:00.190395 Audio: Found 28 devices for driver 'ALSA'
00:00:00.190409 Audio: Device 'Rate Converter Plugin Using Libav/FFmpeg Library':
00:00:00.190410 Audio:   ID              = lavrate
00:00:00.190411 Audio:   Usage           = duplex
00:00:00.190411 Audio:   Flags           = NONE
00:00:00.190411 Audio:   Input channels  = 2
00:00:00.190412 Audio:   Output channels = 2
00:00:00.190415 Audio: Device 'Rate Converter Plugin Using Samplerate Library':
00:00:00.190416 Audio:   ID              = samplerate
00:00:00.190416 Audio:   Usage           = duplex
00:00:00.190416 Audio:   Flags           = NONE
00:00:00.190417 Audio:   Input channels  = 2
00:00:00.190417 Audio:   Output channels = 2
00:00:00.190429 Audio: Device 'Rate Converter Plugin Using Speex Resampler':
00:00:00.190429 Audio:   ID              = speexrate
00:00:00.190430 Audio:   Usage           = duplex
00:00:00.190430 Audio:   Flags           = NONE
00:00:00.190430 Audio:   Input channels  = 2
00:00:00.190431 Audio:   Output channels = 2
00:00:00.190433 Audio: Device 'JACK Audio Connection Kit':
00:00:00.190434 Audio:   ID              = jack
00:00:00.190434 Audio:   Usage           = duplex
00:00:00.190434 Audio:   Flags           = NONE
00:00:00.190435 Audio:   Input channels  = 2
00:00:00.190435 Audio:   Output channels = 2
00:00:00.190437 Audio: Device 'Open Sound System':
00:00:00.190438 Audio:   ID              = oss
00:00:00.190438 Audio:   Usage           = duplex
00:00:00.190439 Audio:   Flags           = NONE
00:00:00.190439 Audio:   Input channels  = 2
00:00:00.190440 Audio:   Output channels = 2
00:00:00.190442 Audio: Device 'PipeWire Sound Server':
00:00:00.190442 Audio:   ID              = pipewire
00:00:00.190442 Audio:   Usage           = duplex
00:00:00.190443 Audio:   Flags           = NONE
00:00:00.190443 Audio:   Input channels  = 2
00:00:00.190444 Audio:   Output channels = 2
00:00:00.190446 Audio: Device 'PulseAudio Sound Server':
00:00:00.190446 Audio:   ID              = pulse
00:00:00.190447 Audio:   Usage           = duplex
00:00:00.190447 Audio:   Flags           = NONE
00:00:00.190447 Audio:   Input channels  = 2
00:00:00.190448 Audio:   Output channels = 2
00:00:00.190450 Audio: Device 'Plugin using Speex DSP (resample, agc, denoise, echo, dereverb)':
00:00:00.190451 Audio:   ID              = speex
00:00:00.190451 Audio:   Usage           = duplex
00:00:00.190451 Audio:   Flags           = NONE
00:00:00.190452 Audio:   Input channels  = 2
00:00:00.190452 Audio:   Output channels = 2
00:00:00.190454 Audio: Device 'Plugin for channel upmix (4,6,8)':
00:00:00.190455 Audio:   ID              = upmix
00:00:00.190455 Audio:   Usage           = duplex
00:00:00.190455 Audio:   Flags           = NONE
00:00:00.190456 Audio:   Input channels  = 2
00:00:00.190456 Audio:   Output channels = 2
00:00:00.190458 Audio: Device 'Plugin for channel downmix (stereo) with a simple spacialization':
00:00:00.190459 Audio:   ID              = vdownmix
00:00:00.190459 Audio:   Usage           = duplex
00:00:00.190460 Audio:   Flags           = NONE
00:00:00.190460 Audio:   Input channels  = 2
00:00:00.190461 Audio:   Output channels = 2
00:00:00.190463 Audio: Device 'Default ALSA Output (currently PipeWire Media Server)':
00:00:00.190463 Audio:   ID              = default
00:00:00.190464 Audio:   Usage           = duplex
00:00:00.190464 Audio:   Flags           = NONE
00:00:00.190464 Audio:   Input channels  = 2
00:00:00.190465 Audio:   Output channels = 2
00:00:00.190467 Audio: Device 'Default Audio Device (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.190467 Audio:   ID              = sysdefault:CARD=PCH
00:00:00.190468 Audio:   Usage           = duplex
00:00:00.190468 Audio:   Flags           = NONE
00:00:00.190468 Audio:   Input channels  = 2
00:00:00.190469 Audio:   Output channels = 2
00:00:00.190471 Audio: Device 'Front output / input (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.190471 Audio:   ID              = front:CARD=PCH,DEV=0
00:00:00.190472 Audio:   Usage           = duplex
00:00:00.190472 Audio:   Flags           = NONE
00:00:00.190473 Audio:   Input channels  = 2
00:00:00.190473 Audio:   Output channels = 2
00:00:00.190475 Audio: Device '2.1 Surround output to Front and Subwoofer speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.190476 Audio:   ID              = surround21:CARD=PCH,DEV=0
00:00:00.190476 Audio:   Usage           = output
00:00:00.190477 Audio:   Flags           = NONE
00:00:00.190477 Audio:   Input channels  = 0
00:00:00.190478 Audio:   Output channels = 2
00:00:00.190483 Audio: Device '4.0 Surround output to Front and Rear speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.190484 Audio:   ID              = surround40:CARD=PCH,DEV=0
00:00:00.190484 Audio:   Usage           = output
00:00:00.190485 Audio:   Flags           = NONE
00:00:00.190485 Audio:   Input channels  = 0
00:00:00.190486 Audio:   Output channels = 2
00:00:00.190488 Audio: Device '4.1 Surround output to Front, Rear and Subwoofer speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.190489 Audio:   ID              = surround41:CARD=PCH,DEV=0
00:00:00.190489 Audio:   Usage           = output
00:00:00.190490 Audio:   Flags           = NONE
00:00:00.190490 Audio:   Input channels  = 0
00:00:00.190490 Audio:   Output channels = 2
00:00:00.190493 Audio: Device '5.0 Surround output to Front, Center and Rear speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.190493 Audio:   ID              = surround50:CARD=PCH,DEV=0
00:00:00.190493 Audio:   Usage           = output
00:00:00.190494 Audio:   Flags           = NONE
00:00:00.190494 Audio:   Input channels  = 0
00:00:00.190495 Audio:   Output channels = 2
00:00:00.190497 Audio: Device '5.1 Surround output to Front, Center, Rear and Subwoofer speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.190497 Audio:   ID              = surround51:CARD=PCH,DEV=0
00:00:00.190498 Audio:   Usage           = output
00:00:00.190498 Audio:   Flags           = NONE
00:00:00.190498 Audio:   Input channels  = 0
00:00:00.190499 Audio:   Output channels = 2
00:00:00.190501 Audio: Device '7.1 Surround output to Front, Center, Side, Rear and Woofer speakers (HDA Intel PCH, ALC887-VD Analog)':
00:00:00.190501 Audio:   ID              = surround71:CARD=PCH,DEV=0
00:00:00.190502 Audio:   Usage           = output
00:00:00.190502 Audio:   Flags           = NONE
00:00:00.190502 Audio:   Input channels  = 0
00:00:00.190503 Audio:   Output channels = 2
00:00:00.190505 Audio: Device 'USB Stream Output (HDA Intel PCH)':
00:00:00.190505 Audio:   ID              = usbstream:CARD=PCH
00:00:00.190506 Audio:   Usage           = duplex
00:00:00.190506 Audio:   Flags           = NONE
00:00:00.190506 Audio:   Input channels  = 2
00:00:00.190507 Audio:   Output channels = 2
00:00:00.190509 Audio: Device 'HDMI Audio Output (HDA NVidia, Smart TV)':
00:00:00.190509 Audio:   ID              = hdmi:CARD=NVidia,DEV=0
00:00:00.190510 Audio:   Usage           = output
00:00:00.190510 Audio:   Flags           = NONE
00:00:00.190511 Audio:   Input channels  = 0
00:00:00.190511 Audio:   Output channels = 2
00:00:00.190513 Audio: Device 'HDMI Audio Output (HDA NVidia, HDMI 1)':
00:00:00.190514 Audio:   ID              = hdmi:CARD=NVidia,DEV=1
00:00:00.190514 Audio:   Usage           = output
00:00:00.190514 Audio:   Flags           = NONE
00:00:00.190515 Audio:   Input channels  = 0
00:00:00.190515 Audio:   Output channels = 2
00:00:00.190517 Audio: Device 'HDMI Audio Output (HDA NVidia, HDMI 2)':
00:00:00.190518 Audio:   ID              = hdmi:CARD=NVidia,DEV=2
00:00:00.190518 Audio:   Usage           = output
00:00:00.190519 Audio:   Flags           = NONE
00:00:00.190519 Audio:   Input channels  = 0
00:00:00.190519 Audio:   Output channels = 2
00:00:00.190521 Audio: Device 'HDMI Audio Output (HDA NVidia, HDMI 3)':
00:00:00.190522 Audio:   ID              = hdmi:CARD=NVidia,DEV=3
00:00:00.190522 Audio:   Usage           = output
00:00:00.190523 Audio:   Flags           = NONE
00:00:00.190523 Audio:   Input channels  = 0
00:00:00.190524 Audio:   Output channels = 2
00:00:00.190526 Audio: Device 'USB Stream Output (HDA NVidia)':
00:00:00.190526 Audio:   ID              = usbstream:CARD=NVidia
00:00:00.190527 Audio:   Usage           = duplex
00:00:00.190527 Audio:   Flags           = NONE
00:00:00.190527 Audio:   Input channels  = 2
00:00:00.190528 Audio:   Output channels = 2
00:00:00.190530 Audio: Device 'Default Audio Device (USB2.0_Camera, USB Audio)':
00:00:00.190530 Audio:   ID              = sysdefault:CARD=USB20Camera
00:00:00.190531 Audio:   Usage           = input
00:00:00.190531 Audio:   Flags           = NONE
00:00:00.190532 Audio:   Input channels  = 2
00:00:00.190532 Audio:   Output channels = 0
00:00:00.190537 Audio: Device 'Front output / input (USB2.0_Camera, USB Audio)':
00:00:00.190538 Audio:   ID              = front:CARD=USB20Camera,DEV=0
00:00:00.190538 Audio:   Usage           = input
00:00:00.190539 Audio:   Flags           = NONE
00:00:00.190539 Audio:   Input channels  = 2
00:00:00.190540 Audio:   Output channels = 0
00:00:00.190542 Audio: Device 'USB Stream Output (USB2.0_Camera)':
00:00:00.190543 Audio:   ID              = usbstream:CARD=USB20Camera
00:00:00.190543 Audio:   Usage           = duplex
00:00:00.190543 Audio:   Flags           = NONE
00:00:00.190544 Audio:   Input channels  = 2
00:00:00.190544 Audio:   Output channels = 2
00:00:00.190617 AC97: Reset
00:00:00.190620 AC97: Mixer reset (EAID=0x809, EACS=0x9)
00:00:00.190623 AC97: Record select to left=mic, right=mic
00:00:00.190626 Audio Mixer: MUTING master volume of 'AC'97 Mixer' -- channel volumes: ff ff ff ff ff ff ff ff ff ff ff ff
00:00:00.190631 Audio Mixer: MUTING sink 'AC'97 Mixer/Line In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.190636 Audio Mixer: MUTING sink 'AC'97 Mixer/Microphone In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.190639 Audio Mixer: MUTING sink 'AC'97 Mixer/PCM Output' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.190644 Audio Mixer: MUTING sink 'AC'97 Mixer/PCM Output' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.190648 Audio Mixer: MUTING sink 'AC'97 Mixer/Line In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.190652 Audio Mixer: MUTING sink 'AC'97 Mixer/Line In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.190655 Audio Mixer: MUTING sink 'AC'97 Mixer/Microphone In' -- channel volumes: 00 00 00 00 00 00 00 00 00 00 00 00
00:00:00.191278 PGM: The CPU physical address width is 36 bits
00:00:00.191283 PGM: PGMR3InitFinalize: 4 MB PSE mask 0000000fffffffff -> VINF_SUCCESS
00:00:00.191291 TM: TMR3InitFinalize: fTSCModeSwitchAllowed=false
00:00:00.191401 CPUM: Mapped 1.0MiB (1048576 bytes) of RAM using fixed-range MTRRs
00:00:00.191410 CPUM: Mapped 4.0GiB (4294967296 bytes) of RAM using 3 variable-range MTRRs
00:00:00.191540 VMM: Enabled thread-context hooks
00:00:00.191542 VMM: RTThreadPreemptIsPending() can be trusted
00:00:00.191543 VMM: Kernel preemption is possible
00:00:00.191617 HM: Host MSR_IA32_FEATURE_CONTROL = 0x5
00:00:00.191882 HM: fWorldSwitcher=0x30000 (fIbpbOnVmExit=false fIbpbOnVmEntry=false fL1dFlushOnVmEntry=false); fL1dFlushOnSched=true fMdsClearOnVmEntry=false
00:00:00.191887 HM: Using VT-x implementation 3.0
00:00:00.191888 HM: Max resume loops                  = 8192
00:00:00.191888 HM: Host CR0                          = 0x80050033
00:00:00.191889 HM: Host CR4                          = 0x1726f0
00:00:00.191890 HM: Host EFER                         = 0xd01
00:00:00.191890 HM: Host SMM_MONITOR_CTL              = 0x0
00:00:00.191890 HM: Host CORE_CAPABILITIES            = 0x0
00:00:00.191891 HM: Host MEMORY_CTRL                  = 0x0
00:00:00.191891 HM: Host DR6 zero'ed                  = 0xffff0ff0
00:00:00.191892 HM: MSR_IA32_FEATURE_CONTROL          = 0x5
00:00:00.191893 HM:   LOCK
00:00:00.191893 HM:   VMXON
00:00:00.191893 HM: MSR_IA32_VMX_BASIC                = 0xda040000000010
00:00:00.191894 HM:   VMCS id                           = 0x10
00:00:00.191894 HM:   VMCS size                         = 1024 bytes
00:00:00.191895 HM:   VMCS physical address limit       = None
00:00:00.191895 HM:   VMCS memory type                  = Write Back (WB)
00:00:00.191896 HM:   Dual-monitor treatment support    = true
00:00:00.191896 HM:   OUTS & INS instruction-info       = true
00:00:00.191897 HM:   Supports true-capability MSRs     = true
00:00:00.191897 HM:   VM-entry Xcpt error-code optional = false
00:00:00.191897 HM: MSR_IA32_VMX_PINBASED_CTLS        = 0x7f00000016
00:00:00.191898 HM:   EXT_INT_EXIT
00:00:00.191898 HM:   NMI_EXIT
00:00:00.191898 HM:   VIRTUAL_NMI
00:00:00.191899 HM:   PREEMPT_TIMER
00:00:00.191899 HM:   POSTED_INT (must be cleared)
00:00:00.191899 HM: MSR_IA32_VMX_PROCBASED_CTLS       = 0xfff9fffe0401e172
00:00:00.191900 HM:   INT_WINDOW_EXIT
00:00:00.191900 HM:   USE_TSC_OFFSETTING
00:00:00.191900 HM:   HLT_EXIT
00:00:00.191901 HM:   INVLPG_EXIT
00:00:00.191901 HM:   MWAIT_EXIT
00:00:00.191901 HM:   RDPMC_EXIT
00:00:00.191902 HM:   RDTSC_EXIT
00:00:00.191902 HM:   CR3_LOAD_EXIT (must be set)
00:00:00.191902 HM:   CR3_STORE_EXIT (must be set)
00:00:00.191902 HM:   USE_TERTIARY_CTLS (must be cleared)
00:00:00.191903 HM:   CR8_LOAD_EXIT
00:00:00.191903 HM:   CR8_STORE_EXIT
00:00:00.191903 HM:   USE_TPR_SHADOW
00:00:00.191904 HM:   NMI_WINDOW_EXIT
00:00:00.191904 HM:   MOV_DR_EXIT
00:00:00.191904 HM:   UNCOND_IO_EXIT
00:00:00.191904 HM:   USE_IO_BITMAPS
00:00:00.191905 HM:   MONITOR_TRAP_FLAG
00:00:00.191905 HM:   USE_MSR_BITMAPS
00:00:00.191905 HM:   MONITOR_EXIT
00:00:00.191905 HM:   PAUSE_EXIT
00:00:00.191906 HM:   USE_SECONDARY_CTLS
00:00:00.191906 HM: MSR_IA32_VMX_PROCBASED_CTLS2      = 0x8ff00000000
00:00:00.191907 HM:   VIRT_APIC_ACCESS
00:00:00.191907 HM:   EPT
00:00:00.191907 HM:   DESC_TABLE_EXIT
00:00:00.191908 HM:   RDTSCP
00:00:00.191908 HM:   VIRT_X2APIC_MODE
00:00:00.191908 HM:   VPID
00:00:00.191908 HM:   WBINVD_EXIT
00:00:00.191909 HM:   UNRESTRICTED_GUEST
00:00:00.191909 HM:   APIC_REG_VIRT (must be cleared)
00:00:00.191909 HM:   VIRT_INT_DELIVERY (must be cleared)
00:00:00.191910 HM:   PAUSE_LOOP_EXIT (must be cleared)
00:00:00.191910 HM:   RDRAND_EXIT
00:00:00.191910 HM:   INVPCID (must be cleared)
00:00:00.191910 HM:   VMFUNC (must be cleared)
00:00:00.191911 HM:   VMCS_SHADOWING (must be cleared)
00:00:00.191911 HM:   ENCLS_EXIT (must be cleared)
00:00:00.191911 HM:   RDSEED_EXIT (must be cleared)
00:00:00.191912 HM:   PML (must be cleared)
00:00:00.191912 HM:   EPT_XCPT_VE (must be cleared)
00:00:00.191912 HM:   CONCEAL_VMX_FROM_PT (must be cleared)
00:00:00.191913 HM:   XSAVES_XRSTORS (must be cleared)
00:00:00.191913 HM:   PASID_TRANSLATE (must be cleared)
00:00:00.191913 HM:   MODE_BASED_EPT_PERM (must be cleared)
00:00:00.191913 HM:   SPP_EPT (must be cleared)
00:00:00.191914 HM:   PT_EPT (must be cleared)
00:00:00.191914 HM:   TSC_SCALING (must be cleared)
00:00:00.191914 HM:   USER_WAIT_PAUSE (must be cleared)
00:00:00.191914 HM:   PCONFIG (must be cleared)
00:00:00.191915 HM:   ENCLV_EXIT (must be cleared)
00:00:00.191915 HM:   BUS_LOCK_DETECT (must be cleared)
00:00:00.191915 HM:   INSTR_TIMEOUT (must be cleared)
00:00:00.191915 HM: MSR_IA32_VMX_ENTRY_CTLS           = 0xffff000011ff
00:00:00.191916 HM:   LOAD_DEBUG (must be set)
00:00:00.191916 HM:   IA32E_MODE_GUEST
00:00:00.191917 HM:   ENTRY_TO_SMM
00:00:00.191917 HM:   DEACTIVATE_DUAL_MON
00:00:00.191917 HM:   LOAD_PERF_MSR
00:00:00.191917 HM:   LOAD_PAT_MSR
00:00:00.191918 HM:   LOAD_EFER_MSR
00:00:00.191918 HM:   LOAD_BNDCFGS_MSR (must be cleared)
00:00:00.191918 HM:   CONCEAL_VMX_FROM_PT (must be cleared)
00:00:00.191919 HM:   LOAD_RTIT_CTL_MSR (must be cleared)
00:00:00.191919 HM:   LOAD_UINV (must be cleared)
00:00:00.191919 HM:   LOAD_CET_STATE (must be cleared)
00:00:00.191919 HM:   LOAD_LBR_CTL_MSR (must be cleared)
00:00:00.191920 HM:   LOAD_PKRS_MSR (must be cleared)
00:00:00.191920 HM: MSR_IA32_VMX_EXIT_CTLS            = 0x7fffff00036dff
00:00:00.191921 HM:   SAVE_DEBUG (must be set)
00:00:00.191921 HM:   HOST_ADDR_SPACE_SIZE
00:00:00.191921 HM:   LOAD_PERF_MSR
00:00:00.191921 HM:   ACK_EXT_INT
00:00:00.191922 HM:   SAVE_PAT_MSR
00:00:00.191922 HM:   LOAD_PAT_MSR
00:00:00.191922 HM:   SAVE_EFER_MSR
00:00:00.191922 HM:   LOAD_EFER_MSR
00:00:00.191923 HM:   SAVE_PREEMPT_TIMER
00:00:00.191923 HM:   CLEAR_BNDCFGS_MSR (must be cleared)
00:00:00.191923 HM:   CONCEAL_VMX_FROM_PT (must be cleared)
00:00:00.191924 HM:   CLEAR_RTIT_CTL_MSR (must be cleared)
00:00:00.191924 HM:   CLEAR_LBR_CTL_MSR (must be cleared)
00:00:00.191924 HM:   CLEAR_UINV (must be cleared)
00:00:00.191925 HM:   LOAD_CET_STATE (must be cleared)
00:00:00.191925 HM:   LOAD_PKRS_MSR (must be cleared)
00:00:00.191925 HM:   SAVE_PERF_MSR (must be cleared)
00:00:00.191925 HM: MSR_IA32_VMX_TRUE_PINBASED_CTLS   = 0x7f00000016
00:00:00.191926 HM: MSR_IA32_VMX_TRUE_PROCBASED_CTLS  = 0xfff9fffe04006172
00:00:00.191927 HM: MSR_IA32_VMX_TRUE_ENTRY_CTLS      = 0xffff000011fb
00:00:00.191927 HM: MSR_IA32_VMX_TRUE_EXIT_CTLS       = 0x7fffff00036dfb
00:00:00.191928 HM: MSR_IA32_VMX_MISC                 = 0x100401e5
00:00:00.191928 HM:   PREEMPT_TIMER_TSC                 = 0x5
00:00:00.191929 HM:   EXIT_SAVE_EFER_LMA                = true
00:00:00.191929 HM:   ACTIVITY_STATES                   = 0x7 ( HLT SHUTDOWN SIPI_WAIT )
00:00:00.191930 HM:   INTEL_PT                          = false
00:00:00.191930 HM:   SMM_READ_SMBASE_MSR               = false
00:00:00.191930 HM:   CR3_TARGET                        = 0x4
00:00:00.191931 HM:   MAX_MSR                           = 0x0 ( 512 )
00:00:00.191931 HM:   VMXOFF_BLOCK_SMI                  = true
00:00:00.191932 HM:   VMWRITE_ALL                       = false
00:00:00.191932 HM:   ENTRY_INJECT_SOFT_INT             = 0x0
00:00:00.191932 HM:   MSEG_ID                           = 0x0
00:00:00.191933 HM: MSR_IA32_VMX_VMCS_ENUM            = 0x2a
00:00:00.191933 HM:   HIGHEST_IDX                       = 0x15
00:00:00.191933 HM: MSR_IA32_VMX_EPT_VPID_CAP         = 0xf0106114141
00:00:00.191934 HM:   RWX_X_ONLY
00:00:00.191934 HM:   PAGE_WALK_LENGTH_4
00:00:00.191935 HM:   MEMTYPE_UC
00:00:00.191935 HM:   MEMTYPE_WB
00:00:00.191935 HM:   PDE_2M
00:00:00.191935 HM:   INVEPT
00:00:00.191936 HM:   INVEPT_SINGLE_CONTEXT
00:00:00.191936 HM:   INVEPT_ALL_CONTEXTS
00:00:00.191936 HM:   INVVPID
00:00:00.191936 HM:   INVVPID_INDIV_ADDR
00:00:00.191937 HM:   INVVPID_SINGLE_CONTEXT
00:00:00.191937 HM:   INVVPID_ALL_CONTEXTS
00:00:00.191937 HM:   INVVPID_SINGLE_CONTEXT_RETAIN_GLOBALS
00:00:00.191937 HM: MSR_IA32_VMX_CR0_FIXED0           = 0x80000021
00:00:00.191938 HM: MSR_IA32_VMX_CR0_FIXED1           = 0xffffffff
00:00:00.191938 HM: MSR_IA32_VMX_CR4_FIXED0           = 0x2000
00:00:00.191939 HM: MSR_IA32_VMX_CR4_FIXED1           = 0x1767ff
00:00:00.191939 HM: Guest support: 32-bit only
00:00:00.191944 HM: Supports VMCS EFER fields         = true
00:00:00.191945 HM: Enabled VMX
00:00:00.191947 HM: Enabled nested paging
00:00:00.191948 HM:   EPT flush type                  = Single context
00:00:00.191948 HM: Enabled unrestricted guest execution
00:00:00.191948 HM: Enabled VPID
00:00:00.191948 HM:   VPID flush type                 = Single context
00:00:00.191949 HM: Enabled VMX-preemption timer (cPreemptTimerShift=5)
00:00:00.191949 HM: VT-x/AMD-V init method: Global
00:00:00.191950 HM: VT-x/AMD-V enable method: Host API
00:00:00.191950 EM: Exit history optimizations: enabled=true enabled-r0=true enabled-r0-no-preemption=false
00:00:00.191966 PcBios: ATA LUN#0 LCHS=520/128/63
00:00:00.191972 APIC: fPostedIntrsEnabled=false fVirtApicRegsEnabled=false fSupportsTscDeadline=false
00:00:00.191974 TMR3UtcNow: nsNow=1 783 449 546 467 973 000 nsPrev=0 -> cNsDelta=1 783 449 546 467 973 000 (offLag=0 offVirtualSync=0 offVirtualSyncGivenUp=0, NowAgain=1 783 449 546 467 973 000)
00:00:00.192002 VMM: fUsePeriodicPreemptionTimers=false
00:00:00.192114 CPUM: Logical host processors: 8 present, 8 max, 8 online, online mask: 00000000000000ff
00:00:00.192208 CPUM: Physical host cores: 4
00:00:00.192208 ************************ CPUID dump *************************
00:00:00.192412          Raw Standard CPUID Leaves
00:00:00.192412      Leaf/sub-leaf  eax      ebx      ecx      edx
00:00:00.192413 Gst: 00000000/0000  0000000d 756e6547 6c65746e 49656e69
00:00:00.192415 Hst:                0000000d 756e6547 6c65746e 49656e69
00:00:00.192415 Gst: 00000001/0000  000306a9 00040800 769a2203 178bfbbf
00:00:00.192416 Hst:                000306a9 02100800 7fbae3ff bfebfbff
00:00:00.192417 Gst: 00000002/0000  76035a01 00f0b2ff 00000000 00ca0000
00:00:00.192418 Hst:                76035a01 00f0b2ff 00000000 00ca0000
00:00:00.192419 Gst: 00000003/0000  00000000 00000000 00000000 00000000
00:00:00.192419 Hst:                00000000 00000000 00000000 00000000
00:00:00.192420 Gst: 00000004/0000  0c000121 01c0003f 0000003f 00000000
00:00:00.192421 Hst:                1c004121 01c0003f 0000003f 00000000
00:00:00.192421 Gst: 00000004/0001  0c000122 01c0003f 0000003f 00000000
00:00:00.192422 Hst:                1c004122 01c0003f 0000003f 00000000
00:00:00.192423 Gst: 00000004/0002  0c000143 01c0003f 000001ff 00000000
00:00:00.192423 Hst:                1c004143 01c0003f 000001ff 00000000
00:00:00.192424 Gst: 00000004/0003  0c000163 03c0003f 00001fff 00000006
00:00:00.192425 Hst:                1c03c163 03c0003f 00001fff 00000006
00:00:00.192425 Gst: 00000004/0004  0c000000 00000000 00000000 00000000
00:00:00.192426 Hst:                00000000 00000000 00000000 00000000
00:00:00.192427 Gst: 00000005/0000  00000000 00000000 00000000 00000000
00:00:00.192427 Hst:                00000040 00000040 00000003 00001120
00:00:00.192428 Gst: 00000006/0000  00000004 00000000 00000000 00000000
00:00:00.192428 Hst:                00000077 00000002 00000009 00000000
00:00:00.192429 Gst: 00000007/0000  00000000 00000001 00000000 10000400
00:00:00.192430 Hst:                00000000 00000281 00000000 9c000400
00:00:00.192430 Gst: 00000007/0001  00000000 00000000 00000000 00000000
00:00:00.192431 Hst:                00000000 00000000 00000000 00000000
00:00:00.192431 Gst: 00000007/0002  00000000 00000000 00000000 00000000
00:00:00.192432 Hst:                00000000 00000000 00000000 00000000
00:00:00.192432 Gst: 00000008/0000  00000000 00000000 00000000 00000000
00:00:00.192433 Hst:                00000000 00000000 00000000 00000000
00:00:00.192433 Gst: 00000009/0000  00000000 00000000 00000000 00000000
00:00:00.192434 Hst:                00000000 00000000 00000000 00000000
00:00:00.192434 Gst: 0000000a/0000  00000000 00000000 00000000 00000000
00:00:00.192435 Hst:                07300403 00000000 00000000 00000603
00:00:00.192436 Gst: 0000000b/0000  00000000 00000001 00000100 00000000
00:00:00.192436 Hst:                00000001 00000002 00000100 00000002
00:00:00.192437 Gst: 0000000b/0001  00000002 00000004 00000201 00000000
00:00:00.192437 Hst:                00000004 00000008 00000201 00000002
00:00:00.192438 Gst: 0000000b/0002  00000000 00000000 00000002 00000000
00:00:00.192439 Hst:                00000000 00000000 00000002 00000002
00:00:00.192439 Gst: 0000000c/0000  00000000 00000000 00000000 00000000
00:00:00.192440 Hst:                00000000 00000000 00000000 00000000
00:00:00.192440 Gst: 0000000d/0000  00000007 00000340 00000340 00000000
00:00:00.192441 Hst:                00000007 00000340 00000340 00000000
00:00:00.192441 Gst: 0000000d/0001  00000000 00000000 00000000 00000000
00:00:00.192442 Hst:                00000001 00000000 00000000 00000000
00:00:00.192442 Gst: 0000000d/0002  00000100 00000240 00000000 00000000
00:00:00.192443 Hst:                00000100 00000240 00000000 00000000
00:00:00.192444 Gst: 0000000d/0003  00000000 00000000 00000000 00000000
00:00:00.192444 Hst:                00000000 00000000 00000000 00000000
00:00:00.192445                                Name: GenuineIntel
00:00:00.192446                            Supports: 0x00000000-0x0000000d
00:00:00.192447                              Family:  6 	Extended: 0 	Effective: 6
00:00:00.192448                               Model: 10 	Extended: 3 	Effective: 58
00:00:00.192449                            Stepping: 9
00:00:00.192449                                Type: 0 (primary)
00:00:00.192450                             APIC ID: 0x00
00:00:00.192451                        Logical CPUs: 4
00:00:00.192452                        CLFLUSH Size: 8
00:00:00.192452                            Brand ID: 0x00
00:00:00.192453 Features
00:00:00.192453   Mnemonic - Description                                  = Guest (Host)
00:00:00.192459   FPU - x87 FPU on Chip                                   = 1 (1)
00:00:00.192461   VME - Virtual 8086 Mode Enhancements                    = 1 (1)
00:00:00.192461   DE - Debugging extensions                               = 1 (1)
00:00:00.192463   PSE - Page Size Extension                               = 1 (1)
00:00:00.192463   TSC - Time Stamp Counter                                = 1 (1)
00:00:00.192464   MSR - Model Specific Registers                          = 1 (1)
00:00:00.192465   PAE - Physical Address Extension                        = 0 (1)
00:00:00.192466   MCE - Machine Check Exception                           = 1 (1)
00:00:00.192467   CX8 - CMPXCHG8B instruction                             = 1 (1)
00:00:00.192467   APIC - APIC On-Chip                                     = 1 (1)
00:00:00.192468   SEP - SYSENTER and SYSEXIT Present                      = 1 (1)
00:00:00.192469   MTRR - Memory Type Range Registers                      = 1 (1)
00:00:00.192470   PGE - PTE Global Bit                                    = 1 (1)
00:00:00.192471   MCA - Machine Check Architecture                        = 1 (1)
00:00:00.192472   CMOV - Conditional Move instructions                    = 1 (1)
00:00:00.192472   PAT - Page Attribute Table                              = 1 (1)
00:00:00.192473   PSE-36 - 36-bit Page Size Extension                     = 1 (1)
00:00:00.192474   PSN - Processor Serial Number                           = 0 (0)
00:00:00.192475   CLFSH - CLFLUSH instruction                             = 1 (1)
00:00:00.192476   DS - Debug Store                                        = 0 (1)
00:00:00.192477   ACPI - Thermal Mon. & Soft. Clock Ctrl.                 = 0 (1)
00:00:00.192477   MMX - Intel MMX Technology                              = 1 (1)
00:00:00.192478   FXSR - FXSAVE and FXRSTOR instructions                  = 1 (1)
00:00:00.192479   SSE - SSE support                                       = 1 (1)
00:00:00.192480   SSE2 - SSE2 support                                     = 1 (1)
00:00:00.192481   SS - Self Snoop                                         = 0 (1)
00:00:00.192482   HTT - Hyper-Threading Technology                        = 1 (1)
00:00:00.192482   TM - Therm. Monitor                                     = 0 (1)
00:00:00.192483   PBE - Pending Break Enabled                             = 0 (1)
00:00:00.192484   SSE3 - SSE3 support                                     = 1 (1)
00:00:00.192485   PCLMUL - PCLMULQDQ support (for AES-GCM)                = 1 (1)
00:00:00.192486   DTES64 - DS Area 64-bit Layout                          = 0 (1)
00:00:00.192487   MONITOR - MONITOR/MWAIT instructions                    = 0 (1)
00:00:00.192487   CPL-DS - CPL Qualified Debug Store                      = 0 (1)
00:00:00.192488   VMX - Virtual Machine Extensions                        = 0 (1)
00:00:00.192489   SMX - Safer Mode Extensions                             = 0 (1)
00:00:00.192490   EST - Enhanced SpeedStep Technology                     = 0 (1)
00:00:00.192491   TM2 - Terminal Monitor 2                                = 0 (1)
00:00:00.192491   SSSE3 - Supplemental Streaming SIMD Extensions 3        = 1 (1)
00:00:00.192492   CNTX-ID - L1 Context ID                                 = 0 (0)
00:00:00.192493   SDBG - Silicon Debug interface                          = 0 (0)
00:00:00.192494   FMA - Fused Multiply Add extensions                     = 0 (0)
00:00:00.192494   CX16 - CMPXCHG16B instruction                           = 1 (1)
00:00:00.192495   TPRUPDATE - xTPR Update Control                         = 0 (1)
00:00:00.192496   PDCM - Perf/Debug Capability MSR                        = 0 (1)
00:00:00.192497   PCID - Process Context Identifiers                      = 1 (1)
00:00:00.192498   DCA - Direct Cache Access                               = 0 (0)
00:00:00.192498   SSE4_1 - SSE4_1 support                                 = 1 (1)
00:00:00.192499   SSE4_2 - SSE4_2 support                                 = 1 (1)
00:00:00.192500   X2APIC - x2APIC support                                 = 0 (1)
00:00:00.192501   MOVBE - MOVBE instruction                               = 0 (0)
00:00:00.192502   POPCNT - POPCNT instruction                             = 1 (1)
00:00:00.192503   TSCDEADL - Time Stamp Counter Deadline                  = 0 (1)
00:00:00.192503   AES - AES instructions                                  = 1 (1)
00:00:00.192504   XSAVE - XSAVE instruction                               = 1 (1)
00:00:00.192505   OSXSAVE - OSXSAVE instruction                           = 0 (1)
00:00:00.192506   AVX - AVX support                                       = 1 (1)
00:00:00.192507   F16C - 16-bit floating point conversion instructions    = 1 (1)
00:00:00.192507   RDRAND - RDRAND instruction                             = 1 (1)
00:00:00.192508   HVP - Hypervisor Present (we're a guest)                = 0 (0)
00:00:00.192509 Structured Extended Feature Flags Enumeration (leaf 7):
00:00:00.192509 Sub-leaf 0
00:00:00.192509   Mnemonic - Description                                  = Guest (Host)
00:00:00.192510   FSGSBASE - RDFSBASE/RDGSBASE/WRFSBASE/WRGSBASE instr.   = 1 (1)
00:00:00.192511   TSCADJUST - Supports MSR_IA32_TSC_ADJUST                = 0 (0)
00:00:00.192512   SGX - Supports Software Guard Extensions                = 0 (0)
00:00:00.192512   BMI1 - Advanced Bit Manipulation extension 1            = 0 (0)
00:00:00.192513   HLE - Hardware Lock Elision                             = 0 (0)
00:00:00.192514   AVX2 - Advanced Vector Extensions 2                     = 0 (0)
00:00:00.192515   FDP_EXCPTN_ONLY - FPU DP only updated on exceptions     = 0 (0)
00:00:00.192515   SMEP - Supervisor Mode Execution Prevention             = 0 (1)
00:00:00.192516   BMI2 - Advanced Bit Manipulation extension 2            = 0 (0)
00:00:00.192517   ERMS - Enhanced REP MOVSB/STOSB instructions            = 0 (1)
00:00:00.192517   INVPCID - INVPCID instruction                           = 0 (0)
00:00:00.192518   RTM - Restricted Transactional Memory                   = 0 (0)
00:00:00.192519   PQM - Platform Quality of Service Monitoring            = 0 (0)
00:00:00.192519   DEPFPU_CS_DS - Deprecates FPU CS, FPU DS values if set  = 0 (0)
00:00:00.192520   MPE - Intel Memory Protection Extensions                = 0 (0)
00:00:00.192520   PQE - Platform Quality of Service Enforcement           = 0 (0)
00:00:00.192521   AVX512F - AVX512 Foundation instructions                = 0 (0)
00:00:00.192522   AVX512DQ - Supports the AVX512DQ instructions           = 0 (0)
00:00:00.192522   RDSEED - RDSEED instruction                             = 0 (0)
00:00:00.192523   ADX - ADCX/ADOX instructions                            = 0 (0)
00:00:00.192524   SMAP - Supervisor Mode Access Prevention                = 0 (0)
00:00:00.192525   AVX512_IFMA - Supports the AVX512_IFMA instructions     = 0 (0)
00:00:00.192525   CLFLUSHOPT - CLFLUSHOPT (Cache Line Flush) instruction  = 0 (0)
00:00:00.192526   CLWB - CLWB instruction                                 = 0 (0)
00:00:00.192527   INTEL_PT - Intel Processor Trace                        = 0 (0)
00:00:00.192527   AVX512PF - AVX512 Prefetch instructions                 = 0 (0)
00:00:00.192528   AVX512ER - AVX512 Exponential & Reciprocal instructions = 0 (0)
00:00:00.192528   AVX512CD - AVX512 Conflict Detection instructions       = 0 (0)
00:00:00.192529   SHA - Secure Hash Algorithm extensions                  = 0 (0)
00:00:00.192530   AVX512BW - Supports the AVX512BW instructions           = 0 (0)
00:00:00.192530   AVX512VL - Supports the AVX512VL instructions           = 0 (0)
00:00:00.192531   PREFETCHWT1 - PREFETCHWT1 instruction                   = 0 (0)
00:00:00.192532   AVX512_VBMI - Supports the AVX512_VBMI instructions     = 0 (0)
00:00:00.192532   UMIP - User mode insturction prevention                 = 0 (0)
00:00:00.192533   PKU - Protection Key for Usermode pages                 = 0 (0)
00:00:00.192534   OSPKE - CR4.PKU mirror                                  = 0 (0)
00:00:00.192535   WAITPKG - TPAUSE, UMONITOR & UMWAIT support             = 0 (0)
00:00:00.192535   AVX512_VBMI2 - Supports the AVX512_VBMI2 instructions   = 0 (0)
00:00:00.192536   CET_SS - CET shadow stack support                       = 0 (0)
00:00:00.192536   GFNI - Supports the GFNI instruction set                = 0 (0)
00:00:00.192537   VAES - Supports the VEX encoded AES instruction set     = 0 (0)
00:00:00.192538   VPCLMULQDQ - Supports the VPCLMULQDQ instruction        = 0 (0)
00:00:00.192538   AVX512_VNNI - Supports the AVX512_VNNI instructions     = 0 (0)
00:00:00.192539   AVX512_BITALG - Supports the AVX512_BITALG instructions = 0 (0)
00:00:00.192539   TME_EN - Supports 4 IA32_TME_ MSRs                      = 0 (0)
00:00:00.192540   AVX512_VPOPCNTDQ - Supports the AVX512_VPOPCNTDQ instructions = 0 (0)
00:00:00.192540   LA57 - 57-bit linear addresses                          = 0 (0)
00:00:00.192541   MAWAU - Value used by BNDLDX & BNDSTX                   = 0x0 (0x0)
00:00:00.192542   RDPID - Read processor ID support                       = 0 (0)
00:00:00.192543   KEY_LOCKER - Supports Key Locker                        = 0 (0)
00:00:00.192544   BUS_LOCK_DETECT - Supports OS bus-lock detection        = 0 (0)
00:00:00.192544   CLDEMOTE - Supports cache line demote                   = 0 (0)
00:00:00.192545   MOVDIRI - Supports the MOVDIRI instruction              = 0 (0)
00:00:00.192546   MOVDIRI64B - Supports the MOVDIRI64B instruction        = 0 (0)
00:00:00.192546   ENQCMD - Supports the Eqnqueue Stores                   = 0 (0)
00:00:00.192547   SGX_LC - Supports SGX Launch Configuration              = 0 (0)
00:00:00.192548   PKS - Supports protection keys for supervisor pages     = 0 (0)
00:00:00.192548   SGX_KEYS - Supports Attestation Service for Intel SGX   = 0 (0)
00:00:00.192549   AVX512_4VNNIW - Supports the AVX512_4VNNIW instructions = 0 (0)
00:00:00.192549   AVX512_4FMAPS - Supports the AVX512_4FMAPS instructions = 0 (0)
00:00:00.192550   FAST_SHORT_REP_MOVSB - Supports fast short REP MOVSB    = 0 (0)
00:00:00.192550   UINTR - Supports user interrupts                        = 0 (0)
00:00:00.192551   AVX512_VP2INTERSECT - Supports the AVX512_VP2INTERSECT instr. = 0 (0)
00:00:00.192551   MCU_OPT_CTRL - Supports IA32_MCU_OPT_CTRL               = 0 (0)
00:00:00.192552   MD_CLEAR - Supports MDS related buffer clearing         = 1 (1)
00:00:00.192553   RTM_ALWAYS_ABORT - XBEGIN always aborts and does fallback = 0 (0)
00:00:00.192553   RTM_FORCE_ABORT - Supports IA32_TSX_FORCE_ABORT         = 0 (0)
00:00:00.192554   SERIALIZE - Supports the SERIALIZE instruction          = 0 (0)
00:00:00.192554   HYBRID - Identifiers the CPU as a hybrid part           = 0 (0)
00:00:00.192555   TSXLDTRK - Supports susp/resume of TSX ld addr tracking = 0 (0)
00:00:00.192556   PCONFIG - Supports the PCONFIG instruction              = 0 (0)
00:00:00.192556   ARCH_LBRS - Supports architectural LBRs                 = 0 (0)
00:00:00.192557   CET_IBT - Supports indirect branch tracking w/ CET      = 0 (0)
00:00:00.192557   AMX_BF16 - Supports tile comp. ops on bfloat16 number   = 0 (0)
00:00:00.192558   AVX512_FP16 - Supports the FP16 data type with AVX512   = 0 (0)
00:00:00.192558   AMX_TILE - Supports the tile architecture               = 0 (0)
00:00:00.192559   AMX_INT8 - Supports tile comp. ops on 8-bit integers    = 0 (0)
00:00:00.192560   IBRS_IBPB - IA32_SPEC_CTRL.IBRS and IA32_PRED_CMD.IBPB  = 0 (1)
00:00:00.192560   STIBP - Supports IA32_SPEC_CTRL.STIBP                   = 0 (1)
00:00:00.192561   FLUSH_CMD - Supports IA32_FLUSH_CMD                     = 1 (1)
00:00:00.192561   ARCHCAP - Supports IA32_ARCH_CAP                        = 0 (0)
00:00:00.192562   CORECAP - Supports IA32_CORE_CAP                        = 0 (0)
00:00:00.192563   SSBD - Supports IA32_SPEC_CTRL.SSBD                     = 0 (1)
00:00:00.192564  Sub-leaf 2
00:00:00.192564   Mnemonic - Description                                  = Guest (Host)
00:00:00.192565   PSFD - Supports IA32_SPEC_CTRL[7] (PSFD)                = 0 (0)
00:00:00.192565   IPRED_CTRL - Supports IA32_SPEC_CTRL[4:3] (IPRED_DIS)   = 0 (0)
00:00:00.192566   RRSBA_CTRL - Supports IA32_SPEC_CTRL[6:5] (RRSBA_DIS)   = 0 (0)
00:00:00.192566   DDPD_U - Supports IA32_SPEC_CTRL[8] (DDPD_U)            = 0 (0)
00:00:00.192567   BHI_CTRL - Supports IA32_SPEC_CTRL[10] (BHI_DIS_S)      = 0 (0)
00:00:00.192568   MCDT_NO - No MXCSR Config Dependent Timing issues       = 0 (0)
00:00:00.192568   UC_LOCK_DIS - Supports UC-lock disable and causing #AC  = 0 (0)
00:00:00.192569   MONITOR_MITG_NO - No MONITOR/UMONITOR power issues      = 0 (0)
00:00:00.192569 Processor Extended State Enumeration (leaf 0xd):
00:00:00.192570    XSAVE area cur/max size by XCR0, Guest: 0x340/0x340
00:00:00.192570    XSAVE area cur/max size by XCR0,  Host: 0x340/0x340
00:00:00.192571                    Valid XCR0 bits, Guest: 0x00000000`00000007 ( x87 SSE YMM_Hi128 )
00:00:00.192573                    Valid XCR0 bits,  Host: 0x00000000`00000007 ( x87 SSE YMM_Hi128 )
00:00:00.192574                     XSAVE features, Guest
00:00:00.192575                     XSAVE features,  Host XSAVEOPT
00:00:00.192576       XSAVE area cur size XCR0|XSS, Guest: 0x0
00:00:00.192577       XSAVE area cur size XCR0|XSS,  Host: 0x0
00:00:00.192577                Valid IA32_XSS bits, Guest: 0x00000000`00000000
00:00:00.192578                Valid IA32_XSS bits,  Host: 0x00000000`00000000
00:00:00.192578   State #2, Guest: off=0x0240, cb=0x0100 IA32_XSS-bit -- YMM_Hi128
00:00:00.192580   State #2,  Host:  off=0x0240, cb=0x0100 IA32_XSS-bit -- YMM_Hi128
00:00:00.192583          Raw Extended CPUID Leaves
00:00:00.192583      Leaf/sub-leaf  eax      ebx      ecx      edx
00:00:00.192584 Gst: 80000000/0000  80000008 00000000 00000000 00000000
00:00:00.192584 Hst:                80000008 00000000 00000000 00000000
00:00:00.192585 Gst: 80000001/0000  00000000 00000000 00000001 08000800
00:00:00.192586 Hst:                00000000 00000000 00000001 28100800
00:00:00.192586 Gst: 80000002/0000  20202020 20202020 65746e49 2952286c
00:00:00.192587 Hst:                20202020 20202020 65746e49 2952286c
00:00:00.192588 Gst: 80000003/0000  726f4320 4d542865 37692029 3737332d
00:00:00.192589 Hst:                726f4320 4d542865 37692029 3737332d
00:00:00.192590 Gst: 80000004/0000  50432030 20402055 30342e33 007a4847
00:00:00.192591 Hst:                50432030 20402055 30342e33 007a4847
00:00:00.192591 Gst: 80000005/0000  00000000 00000000 00000000 00000000
00:00:00.192592 Hst:                00000000 00000000 00000000 00000000
00:00:00.192592 Gst: 80000006/0000  00000000 00000000 01006040 00000000
00:00:00.192593 Hst:                00000000 00000000 01006040 00000000
00:00:00.192594 Gst: 80000007/0000  00000000 00000000 00000000 00000100
00:00:00.192594 Hst:                00000000 00000000 00000000 00000100
00:00:00.192595 Gst: 80000008/0000  00003024 00000000 00000000 00000000
00:00:00.192596 Hst:                00003024 00000000 00000000 00000000
00:00:00.192596 Ext Name:
00:00:00.192597 Ext Supports:                    0x80000000-0x80000008
00:00:00.192597 Family:                          0  	Extended: 0 	Effective: 0
00:00:00.192598 Model:                           0  	Extended: 0 	Effective: 0
00:00:00.192598 Stepping:                        0
00:00:00.192598 Brand ID:                        0x000
00:00:00.192599 Ext Features
00:00:00.192599   Mnemonic - Description                                  = Guest (Host)
00:00:00.192600   FPU - x87 FPU on Chip                                   = 0 (0)
00:00:00.192601   VME - Virtual 8086 Mode Enhancements                    = 0 (0)
00:00:00.192602   DE - Debugging extensions                               = 0 (0)
00:00:00.192603   PSE - Page Size Extension                               = 0 (0)
00:00:00.192604   TSC - Time Stamp Counter                                = 0 (0)
00:00:00.192605   MSR - K86 Model Specific Registers                      = 0 (0)
00:00:00.192605   PAE - Physical Address Extension                        = 0 (0)
00:00:00.192606   MCE - Machine Check Exception                           = 0 (0)
00:00:00.192607   CX8 - CMPXCHG8B instruction                             = 0 (0)
00:00:00.192608   APIC - APIC On-Chip                                     = 0 (0)
00:00:00.192609   SEP - SYSCALL/SYSRET                                    = 1 (1)
00:00:00.192610   MTRR - Memory Type Range Registers                      = 0 (0)
00:00:00.192610   PGE - PTE Global Bit                                    = 0 (0)
00:00:00.192611   MCA - Machine Check Architecture                        = 0 (0)
00:00:00.192612   CMOV - Conditional Move instructions                    = 0 (0)
00:00:00.192613   PAT - Page Attribute Table                              = 0 (0)
00:00:00.192614   PSE-36 - 36-bit Page Size Extension                     = 0 (0)
00:00:00.192614   NX - No-Execute/Execute-Disable                         = 0 (1)
00:00:00.192615   AXMMX - AMD Extensions to MMX instructions              = 0 (0)
00:00:00.192616   MMX - Intel MMX Technology                              = 0 (0)
00:00:00.192617   FXSR - FXSAVE and FXRSTOR Instructions                  = 0 (0)
00:00:00.192617   FFXSR - AMD fast FXSAVE and FXRSTOR instructions        = 0 (0)
00:00:00.192618   Page1GB - 1 GB large page                               = 0 (0)
00:00:00.192619   RDTSCP - RDTSCP instruction                             = 1 (1)
00:00:00.192620   LM - AMD64 Long Mode                                    = 0 (1)
00:00:00.192621   3DNOWEXT - AMD Extensions to 3DNow                      = 0 (0)
00:00:00.192621   3DNOW - AMD 3DNow                                       = 0 (0)
00:00:00.192622   LahfSahf - LAHF/SAHF support in 64-bit mode             = 1 (1)
00:00:00.192623   CmpLegacy - Core multi-processing legacy mode           = 0 (0)
00:00:00.192623   SVM - AMD Secure Virtual Machine extensions             = 0 (0)
00:00:00.192624   EXTAPIC - AMD Extended APIC registers                   = 0 (0)
00:00:00.192625   CR8L - AMD LOCK MOV CR0 means MOV CR8                   = 0 (0)
00:00:00.192626   ABM - AMD Advanced Bit Manipulation                     = 0 (0)
00:00:00.192626   SSE4A - SSE4A instructions                              = 0 (0)
00:00:00.192627   MISALIGNSSE - AMD Misaligned SSE mode                   = 0 (0)
00:00:00.192628   3DNOWPRF - AMD PREFETCH and PREFETCHW instructions      = 0 (0)
00:00:00.192628   OSVW - AMD OS Visible Workaround                        = 0 (0)
00:00:00.192629   IBS - Instruct Based Sampling                           = 0 (0)
00:00:00.192630   XOP - Extended Operation support                        = 0 (0)
00:00:00.192631   SKINIT - SKINIT, STGI, and DEV support                  = 0 (0)
00:00:00.192631   WDT - AMD Watchdog Timer support                        = 0 (0)
00:00:00.192632   LWP - Lightweight Profiling support                     = 0 (0)
00:00:00.192633   FMA4 - Four operand FMA instruction support             = 0 (0)
00:00:00.192633   TCE - Translation Cache Extension support               = 0 (0)
00:00:00.192634   NodeId - NodeId in MSR C001_100C                        = 0 (0)
00:00:00.192635   TBM - Trailing Bit Manipulation instructions            = 0 (0)
00:00:00.192635   TOPOEXT - Topology Extensions                           = 0 (0)
00:00:00.192636   PRFEXTCORE - Performance Counter Extensions support     = 0 (0)
00:00:00.192637   PRFEXTNB - NB Performance Counter Extensions support    = 0 (0)
00:00:00.192637   DATABPEXT - Data-access Breakpoint Extension            = 0 (0)
00:00:00.192638   PERFTSC - Performance Time Stamp Counter                = 0 (0)
00:00:00.192639   PCX_L2I - L2I/L3 Performance Counter Extensions         = 0 (0)
00:00:00.192639   MONITORX - MWAITX and MONITORX instructions             = 0 (0)
00:00:00.192640   AddrMaskExt - BP Addressing masking extended to bit 31  = 0 (0)
00:00:00.192640 Full Name:                       "        Intel(R) Core(TM) i7-3770 CPU @ 3.40GHz"
00:00:00.192641 TLB 2/4M Instr/Uni:              res0     0 entries
00:00:00.192641 TLB 2/4M Data:                   res0     0 entries
00:00:00.192642 TLB 4K Instr/Uni:                res0     0 entries
00:00:00.192642 TLB 4K Data:                     res0     0 entries
00:00:00.192642 L1 Instr Cache Line Size:        0 bytes
00:00:00.192643 L1 Instr Cache Lines Per Tag:    0
00:00:00.192643 L1 Instr Cache Associativity:    res0
00:00:00.192643 L1 Instr Cache Size:             0 KB
00:00:00.192644 L1 Data Cache Line Size:         0 bytes
00:00:00.192644 L1 Data Cache Lines Per Tag:     0
00:00:00.192644 L1 Data Cache Associativity:     res0
00:00:00.192644 L1 Data Cache Size:              0 KB
00:00:00.192645 L2 TLB 2/4M Instr/Uni:           off       0 entries
00:00:00.192645 L2 TLB 2/4M Data:                off       0 entries
00:00:00.192646 L2 TLB 4K Instr/Uni:             off       0 entries
00:00:00.192646 L2 TLB 4K Data:                  off       0 entries
00:00:00.192646 L2 Cache Line Size:              64 bytes
00:00:00.192647 L2 Cache Lines Per Tag:          0
00:00:00.192647 L2 Cache Associativity:          8 way
00:00:00.192647 L2 Cache Size:                   256 KB
00:00:00.192647 L3 Cache Line Size:              0 bytes
00:00:00.192648 L3 Cache Lines Per Tag:          0
00:00:00.192648 L3 Cache Associativity:          off
00:00:00.192648 L3 Cache Size:                   0 KB
00:00:00.192649 APM Features EDX
00:00:00.192649   Mnemonic - Description                                  = Guest (Host)
00:00:00.192650   TS - Temperature Sensor                                 = 0 (0)
00:00:00.192651   FID - Frequency ID control                              = 0 (0)
00:00:00.192652   VID - Voltage ID control                                = 0 (0)
00:00:00.192652   TTP - Thermal Trip                                      = 0 (0)
00:00:00.192653   TM - Hardware Thermal Control (HTC)                     = 0 (0)
00:00:00.192654   100MHzSteps - 100 MHz Multiplier control                = 0 (0)
00:00:00.192655   HwPstate - Hardware P-state control                     = 0 (0)
00:00:00.192655   TscInvariant - Invariant Time Stamp Counter             = 1 (1)
00:00:00.192656   CPB - Core Performance Boost                            = 0 (0)
00:00:00.192657   EffFreqRO - Read-only Effective Frequency Interface     = 0 (0)
00:00:00.192657   ProcFdbkIf - Processor Feedback Interface               = 0 (0)
00:00:00.192658   ProcPwrRep - Core power reporting interface support     = 0 (0)
00:00:00.192659   ConnectedStandby - Connected Standby                    = 0 (0)
00:00:00.192659   RAPL - Running average power limit                      = 0 (0)
00:00:00.192660 Physical Address Width:          36 bits
00:00:00.192660 Virtual Address Width:           48 bits
00:00:00.192661 Max page count for INVLPGB:      0x3024
00:00:00.192661 Max ECX for RDPRU:               0x0
00:00:00.192662 ********************* End of CPUID dump *********************
00:00:00.192663 *********************** VT-x features ***********************
00:00:00.192664 Nested hardware virtualization - VMX features
00:00:00.192664   Mnemonic - Description                                  = guest (host)
00:00:00.192664   VMX - Virtual-Machine Extensions                        = 0 (1)
00:00:00.192665   InsOutInfo - INS/OUTS instruction info.                 = 0 (1)
00:00:00.192665   ExtIntExit - External interrupt exiting                 = 0 (1)
00:00:00.192666   NmiExit - NMI exiting                                   = 0 (1)
00:00:00.192666   VirtNmi - Virtual NMIs                                  = 0 (1)
00:00:00.192666   PreemptTimer - VMX preemption timer                     = 0 (1)
00:00:00.192667   PostedInt - Posted interrupts                           = 0 (0)
00:00:00.192667   IntWindowExit - Interrupt-window exiting                = 0 (1)
00:00:00.192668   TscOffsetting - TSC offsetting                          = 0 (1)
00:00:00.192668   HltExit - HLT exiting                                   = 0 (1)
00:00:00.192668   InvlpgExit - INVLPG exiting                             = 0 (1)
00:00:00.192669   MwaitExit - MWAIT exiting                               = 0 (1)
00:00:00.192669   RdpmcExit - RDPMC exiting                               = 0 (1)
00:00:00.192669   RdtscExit - RDTSC exiting                               = 0 (1)
00:00:00.192670   Cr3LoadExit - CR3-load exiting                          = 0 (1)
00:00:00.192670   Cr3StoreExit - CR3-store exiting                        = 0 (1)
00:00:00.192671   TertiaryExecCtls - Activate tertiary controls           = 0 (0)
00:00:00.192671   Cr8LoadExit  - CR8-load exiting                         = 0 (1)
00:00:00.192671   Cr8StoreExit - CR8-store exiting                        = 0 (1)
00:00:00.192672   UseTprShadow - Use TPR shadow                           = 0 (1)
00:00:00.192672   NmiWindowExit - NMI-window exiting                      = 0 (1)
00:00:00.192673   MovDRxExit - Mov-DR exiting                             = 0 (1)
00:00:00.192673   UncondIoExit - Unconditional I/O exiting                = 0 (1)
00:00:00.192673   UseIoBitmaps - Use I/O bitmaps                          = 0 (1)
00:00:00.192674   MonitorTrapFlag - Monitor Trap Flag                     = 0 (1)
00:00:00.192674   UseMsrBitmaps - MSR bitmaps                             = 0 (1)
00:00:00.192674   MonitorExit - MONITOR exiting                           = 0 (1)
00:00:00.192675   PauseExit - PAUSE exiting                               = 0 (1)
00:00:00.192675   SecondaryExecCtl - Activate secondary controls          = 0 (1)
00:00:00.192676   VirtApic - Virtualize-APIC accesses                     = 0 (1)
00:00:00.192676   Ept - Extended Page Tables                              = 0 (1)
00:00:00.192676   DescTableExit - Descriptor-table exiting                = 0 (1)
00:00:00.192677   Rdtscp - Enable RDTSCP                                  = 0 (1)
00:00:00.192677   VirtX2ApicMode - Virtualize-x2APIC mode                 = 0 (1)
00:00:00.192678   Vpid - Enable VPID                                      = 0 (1)
00:00:00.192678   WbinvdExit - WBINVD exiting                             = 0 (1)
00:00:00.192678   UnrestrictedGuest - Unrestricted guest                  = 0 (1)
00:00:00.192679   ApicRegVirt - APIC-register virtualization              = 0 (0)
00:00:00.192679   VirtIntDelivery - Virtual-interrupt delivery            = 0 (0)
00:00:00.192679   PauseLoopExit - PAUSE-loop exiting                      = 0 (0)
00:00:00.192680   RdrandExit - RDRAND exiting                             = 0 (1)
00:00:00.192680   Invpcid - Enable INVPCID                                = 0 (0)
00:00:00.192681   VmFuncs - Enable VM Functions                           = 0 (0)
00:00:00.192681   VmcsShadowing - VMCS shadowing                          = 0 (0)
00:00:00.192681   RdseedExiting - RDSEED exiting                          = 0 (0)
00:00:00.192682   PML - Page-Modification Log                             = 0 (0)
00:00:00.192682   EptVe - EPT violations can cause #VE                    = 0 (0)
00:00:00.192683   ConcealVmxFromPt - Conceal VMX from Processor Trace     = 0 (0)
00:00:00.192683   XsavesXRstors - Enable XSAVES/XRSTORS                   = 0 (0)
00:00:00.192683   PasidTranslate - PASID translation                      = 0 (0)
00:00:00.192684   ModeBasedExecuteEpt - Mode-based execute permissions    = 0 (0)
00:00:00.192684   SppEpt - Sub-page page write permissions for EPT        = 0 (0)
00:00:00.192684   PtEpt - Processor Trace address' translatable by EPT    = 0 (0)
00:00:00.192685   UseTscScaling - Use TSC scaling                         = 0 (0)
00:00:00.192685   UserWaitPause - Enable TPAUSE, UMONITOR and UMWAIT      = 0 (0)
00:00:00.192686   Pconfig - Enable PCONFIG                                = 0 (0)
00:00:00.192686   EnclvExit - ENCLV exiting                               = 0 (0)
00:00:00.192686   BusLockDetect - VMM Bus-Lock detection                  = 0 (0)
00:00:00.192687   InstrTimeout - Instruction timeout                      = 0 (0)
00:00:00.192687   LoadIwKeyExit - LOADIWKEY exiting                       = 0 (0)
00:00:00.192687   HLAT - Hypervisor-managed linear-address translation    = 0 (0)
00:00:00.192688   EptPagingWrite - EPT paging-write                       = 0 (0)
00:00:00.192688   GstPagingVerify - Guest-paging verification             = 0 (0)
00:00:00.192689   IpiVirt - IPI virtualization                            = 0 (0)
00:00:00.192689   VirtSpecCtrl - Virtualize IA32_SPEC_CTRL                = 0 (0)
00:00:00.192689   EntryLoadDebugCtls - Load debug controls on VM-entry    = 0 (1)
00:00:00.192690   Ia32eModeGuest - IA-32e mode guest                      = 0 (1)
00:00:00.192690   EntryLoadEferMsr - Load IA32_EFER MSR on VM-entry       = 0 (1)
00:00:00.192691   EntryLoadPatMsr - Load IA32_PAT MSR on VM-entry         = 0 (1)
00:00:00.192691   ExitSaveDebugCtls - Save debug controls on VM-exit      = 0 (1)
00:00:00.192691   HostAddrSpaceSize - Host address-space size             = 0 (1)
00:00:00.192692   ExitAckExtInt - Acknowledge interrupt on VM-exit        = 0 (1)
00:00:00.192692   ExitSavePatMsr - Save IA32_PAT MSR on VM-exit           = 0 (1)
00:00:00.192692   ExitLoadPatMsr - Load IA32_PAT MSR on VM-exit           = 0 (1)
00:00:00.192693   ExitSaveEferMsr - Save IA32_EFER MSR on VM-exit         = 0 (1)
00:00:00.192693   ExitLoadEferMsr - Load IA32_EFER MSR on VM-exit         = 0 (1)
00:00:00.192694   SavePreemptTimer - Save VMX-preemption timer            = 0 (1)
00:00:00.192694   SecondaryExitCtls - Secondary VM-exit controls          = 0 (0)
00:00:00.192694   ExitSaveEferLma - Save IA32_EFER.LMA on VM-exit         = 0 (1)
00:00:00.192695   IntelPt - Intel Processor Trace in VMX operation        = 0 (0)
00:00:00.192695   VmwriteAll - VMWRITE to any supported VMCS field        = 0 (0)
00:00:00.192695   EntryInjectSoftInt - Inject softint. with 0-len instr.  = 0 (0)
00:00:00.192696
00:00:00.192696 ******************* End of VT-x features ********************
00:00:00.192784 VMEmt: Halt method global1 (5)
00:00:00.192832 VMEmt: HaltedGlobal1 config: cNsSpinBlockThresholdCfg=2000
00:00:00.192872 Changing the VM state from 'CREATING' to 'CREATED'
00:00:00.197001 NAT: DNS settings changed, triggering update
00:00:00.197011 Nameserver is either on 127/8 network or failed to obtain from host. Falling back to libslirp DNS proxy.
00:00:00.197014 fallback virtual nameserver: 50462730Changing the VM state from 'CREATED' to 'POWERING_ON'
00:00:00.197884 Changing the VM state from 'POWERING_ON' to 'RUNNING'
00:00:00.197891 Console: Machine state changed to 'Running'
00:00:00.198439 VBoxHeadless: starting event loop
00:00:00.199024 VMMDev: Guest Log: BIOS: VirtualBox 7.2.12
00:00:00.199097 PCI: Setting up resources and interrupts
00:00:00.202339 PIT: mode=2 count=0x10000 (65536) - 18.20 Hz (ch=0)
00:00:00.225760 Display::i_handleDisplayResize: uScreenId=0 pvVRAM=0000000000000000 w=720 h=400 bpp=0 cbLine=0x0 flags=0x0 origin=0,0
00:00:00.225974 VMMDev: Guest Log: CPUID EDX: 0x178bfbbf
00:00:00.226036 PIIX3 ATA: Ctl#0: RESET, DevSel=0 AIOIf=0 CmdIf0=0x00 (-1 usec ago) CmdIf1=0x00 (-1 usec ago)
00:00:00.226071 PIIX3 ATA: Ctl#0: finished processing RESET
00:00:00.226377 VMMDev: Guest Log: BIOS: ata0-0: PCHS=4161/16/63 LCHS=520/128/63
00:00:00.226813 PIIX3 ATA: Ctl#1: RESET, DevSel=0 AIOIf=0 CmdIf0=0x00 (-1 usec ago) CmdIf1=0x00 (-1 usec ago)
00:00:00.227102 PIIX3 ATA: Ctl#1: finished processing RESET
00:00:00.230294 PIT: mode=2 count=0x48d3 (18643) - 64.00 Hz (ch=0)
00:00:00.257304 Display::i_handleDisplayResize: uScreenId=0 pvVRAM=00007f4a14000000 w=640 h=480 bpp=32 cbLine=0xA00 flags=0x0 origin=0,0
00:00:02.714712 Display::i_handleDisplayResize: uScreenId=0 pvVRAM=0000000000000000 w=720 h=400 bpp=0 cbLine=0x0 flags=0x0 origin=0,0
00:00:02.715704 PIT: mode=2 count=0x10000 (65536) - 18.20 Hz (ch=0)
00:00:02.716067 VMMDev: Guest Log: BIOS: Boot : bseqnr=1, bootseq=0231
00:00:02.716396 VMMDev: Guest Log: BIOS: Boot from Floppy 0 failed
00:00:02.716731 VMMDev: Guest Log: BIOS: Boot : bseqnr=2, bootseq=0023
00:00:02.717261 VMMDev: Guest Log: BIOS: Booting from CD-ROM...
00:00:02.834476 Changing the VM state from 'RUNNING' to 'GURU_MEDITATION'
00:00:02.834502 Console: Machine state changed to 'Stuck'
00:00:02.835546 !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
00:00:02.835548 !!
00:00:02.835549 !!         VCPU0: Guru Meditation 1155 (VINF_EM_TRIPLE_FAULT)
00:00:02.835556 !!
00:00:02.835562 !! Skipping ring-0 registers and stack, rcErr=VINF_EM_TRIPLE_FAULT
00:00:02.835567 !!
00:00:02.835567 !! {mappings, <NULL>}
00:00:02.835569 !!
00:00:02.835582 !!
00:00:02.835582 !! {hma, <NULL>}
00:00:02.835583 !!
00:00:02.835586 !!
00:00:02.835587 !! {cpumguest, verbose}
00:00:02.835588 !!
00:00:02.835603 Guest CPUM (VCPU 0) state:
00:00:02.835606 eax=00000030 ebx=00245a00 ecx=00000000 edx=000003f8 esi=00000000 edi=00108000
00:00:02.835608 eip=001010f4 esp=00007c00 ebp=36d76289 iopl=0  iopl=0 nv up di pl nz na po nc
00:00:02.835609 cs={0008 base=0000000000000000 limit=ffffffff flags=0000c09b} dr0=00000000 dr1=00000000
00:00:02.835613 ds={0010 base=0000000000000000 limit=ffffffff flags=0000c093} dr2=00000000 dr3=00000000
00:00:02.835615 es={0010 base=0000000000000000 limit=ffffffff flags=0000c093} dr4=00000000 dr5=00000000
00:00:02.835617 fs={0010 base=0000000000000000 limit=ffffffff flags=0000c093} dr6=ffff0ff0 dr7=00000400
00:00:02.835619 gs={0010 base=0000000000000000 limit=ffffffff flags=0000c093} cr0=00000011 cr2=00000000
00:00:02.835620 ss={0010 base=0000000000000000 limit=ffffffff flags=0000c093} cr3=00000000 cr4=00000000
00:00:02.835622 gdtr=0000000000101130:0017  idtr=0000000000000000:0000  eflags=00000006
00:00:02.835624 ldtr={0000 base=00000000 limit=0000ffff flags=00000082}
00:00:02.835625 tr  ={0000 base=00000000 limit=0000ffff flags=0000008b}
00:00:02.835626 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.835638 xcr=0000000000000001 xcr1=0000000000000000 xss=0000000000000000 (fXStateMask=0000000000000000)
00:00:02.835640 FCW=037f FSW=0000 FTW=0000 FOP=0000 MXCSR=00001f80 MXCSR_MASK=0000ffff
00:00:02.835642 FPUIP=00000000 CS=0000 Rsrvd1=0000  FPUDP=00000000 DS=0000 Rsvrd2=0000
00:00:02.835644 ST(0)=FPR0={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835649 ST(1)=FPR1={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835652 ST(2)=FPR2={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835655 ST(3)=FPR3={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835658 ST(4)=FPR4={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835661 ST(5)=FPR5={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835664 ST(6)=FPR6={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835666 ST(7)=FPR7={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835668 XMM0 =00000000'00000000'00000000'00000000  XMM1 =00000000'00000000'00000000'00000000
00:00:02.835671 XMM2 =00000000'00000000'00000000'00000000  XMM3 =00000000'00000000'00000000'00000000
00:00:02.835674 XMM4 =00000000'00000000'00000000'00000000  XMM5 =00000000'00000000'00000000'00000000
00:00:02.835676 XMM6 =00000000'00000000'00000000'00000000  XMM7 =00000000'00000000'00000000'00000000
00:00:02.835678 XMM8 =00000000'00000000'00000000'00000000  XMM9 =00000000'00000000'00000000'00000000
00:00:02.835681 XMM10=00000000'00000000'00000000'00000000  XMM11=00000000'00000000'00000000'00000000
00:00:02.835685 XMM12=00000000'00000000'00000000'00000000  XMM13=00000000'00000000'00000000'00000000
00:00:02.835689 XMM14=00000000'00000000'00000000'00000000  XMM15=00000000'00000000'00000000'00000000
00:00:02.835694 EFER         =0000000000000000
00:00:02.835694 PAT          =0007040600070406
00:00:02.835695 STAR         =0000000000000000
00:00:02.835696 CSTAR        =0000000000000000
00:00:02.835696 LSTAR        =0000000000000000
00:00:02.835697 SFMASK       =0000000000000000
00:00:02.835697 KERNELGSBASE =0000000000000000
00:00:02.835700 MTRR_CAP          =0000000000000510
00:00:02.835701 MTRR_DEF_TYPE     =0000000000000c00
00:00:02.835702 MTRR_FIX64K_00000 =0606060606060606
00:00:02.835702 MTRR_FIX16K_80000 =0606060606060606
00:00:02.835703 MTRR_FIX16K_A0000 =0000000000000000
00:00:02.835704 MTRR_FIX4K_C0000  =0505050505050505
00:00:02.835705 MTRR_FIX4K_C8000  =0505050505050505
00:00:02.835705 MTRR_FIX4K_D0000  =0505050505050505
00:00:02.835706 MTRR_FIX4K_D8000  =0505050505050505
00:00:02.835707 MTRR_FIX4K_E0000  =0505050505050505
00:00:02.835708 MTRR_FIX4K_E8000  =0505050505050505
00:00:02.835708 MTRR_FIX4K_F0000  =0505050505050505
00:00:02.835709 MTRR_FIX4K_F8000  =0505050505050505
00:00:02.835717 MTRR_PHYSBASE[ 0] =0000000000000006 First=0000000000000000      0 MB [WB]
00:00:02.835719 MTRR_PHYSMASK[ 0] =0000000f00000800 Last =00000000ffffffff   4095 MB [4096 MB]
00:00:02.835722 MTRR_PHYSBASE[ 1] =00000000e0000000 First=00000000e0000000   3584 MB [UC]
00:00:02.835723 MTRR_PHYSMASK[ 1] =0000000fe0000800 Last =00000000ffffffff   4095 MB [512 MB]
00:00:02.835726 MTRR_PHYSBASE[ 2] =0000000100000006 First=0000000100000000   4096 MB [WB]
00:00:02.835727 MTRR_PHYSMASK[ 2] =0000000fe0000800 Last =000000011fffffff   4607 MB [512 MB]
00:00:02.835730 MTRR_PHYSBASE[ 3] =0000000000000000
00:00:02.835731 MTRR_PHYSMASK[ 3] =0000000000000000
00:00:02.835732 MTRR_PHYSBASE[ 4] =0000000000000000
00:00:02.835733 MTRR_PHYSMASK[ 4] =0000000000000000
00:00:02.835734 MTRR_PHYSBASE[ 5] =0000000000000000
00:00:02.835735 MTRR_PHYSMASK[ 5] =0000000000000000
00:00:02.835737 MTRR_PHYSBASE[ 6] =0000000000000000
00:00:02.835738 MTRR_PHYSMASK[ 6] =0000000000000000
00:00:02.835739 MTRR_PHYSBASE[ 7] =0000000000000000
00:00:02.835740 MTRR_PHYSMASK[ 7] =0000000000000000
00:00:02.835741 MTRR_PHYSBASE[ 8] =0000000000000000
00:00:02.835741 MTRR_PHYSMASK[ 8] =0000000000000000
00:00:02.835742 MTRR_PHYSBASE[ 9] =0000000000000000
00:00:02.835743 MTRR_PHYSMASK[ 9] =0000000000000000
00:00:02.835744 MTRR_PHYSBASE[10] =0000000000000000
00:00:02.835745 MTRR_PHYSMASK[10] =0000000000000000
00:00:02.835746 MTRR_PHYSBASE[11] =0000000000000000
00:00:02.835747 MTRR_PHYSMASK[11] =0000000000000000
00:00:02.835748 MTRR_PHYSBASE[12] =0000000000000000
00:00:02.835749 MTRR_PHYSMASK[12] =0000000000000000
00:00:02.835750 MTRR_PHYSBASE[13] =0000000000000000
00:00:02.835751 MTRR_PHYSMASK[13] =0000000000000000
00:00:02.835752 MTRR_PHYSBASE[14] =0000000000000000
00:00:02.835753 MTRR_PHYSMASK[14] =0000000000000000
00:00:02.835754 MTRR_PHYSBASE[15] =0000000000000000
00:00:02.835754 MTRR_PHYSMASK[15] =0000000000000000
00:00:02.835794 Guest CPUM (VCPU 1) state:
00:00:02.835800 eax=00000000 ebx=00000000 ecx=00000000 edx=00000600 esi=00000000 edi=00000000
00:00:02.835811 eip=0000fff0 esp=00000000 ebp=00000000 iopl=0  iopl=0 nv up di pl nz na pe nc
00:00:02.835813 cs={f000 base=00000000ffff0000 limit=0000ffff flags=0000009b} dr0=00000000 dr1=00000000
00:00:02.835815 ds={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr2=00000000 dr3=00000000
00:00:02.835817 es={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr4=00000000 dr5=00000000
00:00:02.835819 fs={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr6=ffff0ff0 dr7=00000400
00:00:02.835820 gs={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr0=60000010 cr2=00000000
00:00:02.835835 ss={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr3=00000000 cr4=00000000
00:00:02.835837 gdtr=0000000000000000:ffff  idtr=0000000000000000:ffff  eflags=00000002
00:00:02.835838 ldtr={0000 base=00000000 limit=0000ffff flags=00000082}
00:00:02.835839 tr  ={0000 base=00000000 limit=0000ffff flags=0000008b}
00:00:02.835840 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.835852 xcr=0000000000000001 xcr1=0000000000000000 xss=0000000000000000 (fXStateMask=0000000000000000)
00:00:02.835855 FCW=037f FSW=0000 FTW=0000 FOP=0000 MXCSR=00001f80 MXCSR_MASK=0000ffff
00:00:02.835857 FPUIP=00000000 CS=0000 Rsrvd1=0000  FPUDP=00000000 DS=0000 Rsvrd2=0000
00:00:02.835860 ST(0)=FPR0={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835865 ST(1)=FPR1={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835869 ST(2)=FPR2={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835870 ST(3)=FPR3={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835872 ST(4)=FPR4={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835874 ST(5)=FPR5={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835876 ST(6)=FPR6={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835877 ST(7)=FPR7={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.835880 XMM0 =00000000'00000000'00000000'00000000  XMM1 =00000000'00000000'00000000'00000000
00:00:02.835885 XMM2 =00000000'00000000'00000000'00000000  XMM3 =00000000'00000000'00000000'00000000
00:00:02.835889 XMM4 =00000000'00000000'00000000'00000000  XMM5 =00000000'00000000'00000000'00000000
00:00:02.835893 XMM6 =00000000'00000000'00000000'00000000  XMM7 =00000000'00000000'00000000'00000000
00:00:02.835896 XMM8 =00000000'00000000'00000000'00000000  XMM9 =00000000'00000000'00000000'00000000
00:00:02.835900 XMM10=00000000'00000000'00000000'00000000  XMM11=00000000'00000000'00000000'00000000
00:00:02.835904 XMM12=00000000'00000000'00000000'00000000  XMM13=00000000'00000000'00000000'00000000
00:00:02.835908 XMM14=00000000'00000000'00000000'00000000  XMM15=00000000'00000000'00000000'00000000
00:00:02.835911 EFER         =0000000000000000
00:00:02.835911 PAT          =0007040600070406
00:00:02.835912 STAR         =0000000000000000
00:00:02.835912 CSTAR        =0000000000000000
00:00:02.835913 LSTAR        =0000000000000000
00:00:02.835913 SFMASK       =0000000000000000
00:00:02.835913 KERNELGSBASE =0000000000000000
00:00:02.835915 MTRR_CAP          =0000000000000510
00:00:02.835916 MTRR_DEF_TYPE     =0000000000000c00
00:00:02.835916 MTRR_FIX64K_00000 =0606060606060606
00:00:02.835917 MTRR_FIX16K_80000 =0606060606060606
00:00:02.835918 MTRR_FIX16K_A0000 =0000000000000000
00:00:02.835918 MTRR_FIX4K_C0000  =0505050505050505
00:00:02.835918 MTRR_FIX4K_C8000  =0505050505050505
00:00:02.835919 MTRR_FIX4K_D0000  =0505050505050505
00:00:02.835920 MTRR_FIX4K_D8000  =0505050505050505
00:00:02.835920 MTRR_FIX4K_E0000  =0505050505050505
00:00:02.835921 MTRR_FIX4K_E8000  =0505050505050505
00:00:02.835921 MTRR_FIX4K_F0000  =0505050505050505
00:00:02.835922 MTRR_FIX4K_F8000  =0505050505050505
00:00:02.835927 MTRR_PHYSBASE[ 0] =0000000000000006 First=0000000000000000      0 MB [WB]
00:00:02.835928 MTRR_PHYSMASK[ 0] =0000000f00000800 Last =00000000ffffffff   4095 MB [4096 MB]
00:00:02.835932 MTRR_PHYSBASE[ 1] =00000000e0000000 First=00000000e0000000   3584 MB [UC]
00:00:02.835933 MTRR_PHYSMASK[ 1] =0000000fe0000800 Last =00000000ffffffff   4095 MB [512 MB]
00:00:02.835936 MTRR_PHYSBASE[ 2] =0000000100000006 First=0000000100000000   4096 MB [WB]
00:00:02.835937 MTRR_PHYSMASK[ 2] =0000000fe0000800 Last =000000011fffffff   4607 MB [512 MB]
00:00:02.835941 MTRR_PHYSBASE[ 3] =0000000000000000
00:00:02.835942 MTRR_PHYSMASK[ 3] =0000000000000000
00:00:02.835944 MTRR_PHYSBASE[ 4] =0000000000000000
00:00:02.835944 MTRR_PHYSMASK[ 4] =0000000000000000
00:00:02.835946 MTRR_PHYSBASE[ 5] =0000000000000000
00:00:02.835947 MTRR_PHYSMASK[ 5] =0000000000000000
00:00:02.835948 MTRR_PHYSBASE[ 6] =0000000000000000
00:00:02.835949 MTRR_PHYSMASK[ 6] =0000000000000000
00:00:02.835961 MTRR_PHYSBASE[ 7] =0000000000000000
00:00:02.835962 MTRR_PHYSMASK[ 7] =0000000000000000
00:00:02.835963 MTRR_PHYSBASE[ 8] =0000000000000000
00:00:02.835964 MTRR_PHYSMASK[ 8] =0000000000000000
00:00:02.835965 MTRR_PHYSBASE[ 9] =0000000000000000
00:00:02.835966 MTRR_PHYSMASK[ 9] =0000000000000000
00:00:02.835967 MTRR_PHYSBASE[10] =0000000000000000
00:00:02.835967 MTRR_PHYSMASK[10] =0000000000000000
00:00:02.835968 MTRR_PHYSBASE[11] =0000000000000000
00:00:02.835968 MTRR_PHYSMASK[11] =0000000000000000
00:00:02.835969 MTRR_PHYSBASE[12] =0000000000000000
00:00:02.835969 MTRR_PHYSMASK[12] =0000000000000000
00:00:02.835970 MTRR_PHYSBASE[13] =0000000000000000
00:00:02.835971 MTRR_PHYSMASK[13] =0000000000000000
00:00:02.835971 MTRR_PHYSBASE[14] =0000000000000000
00:00:02.835972 MTRR_PHYSMASK[14] =0000000000000000
00:00:02.835972 MTRR_PHYSBASE[15] =0000000000000000
00:00:02.835973 MTRR_PHYSMASK[15] =0000000000000000
00:00:02.836007 Guest CPUM (VCPU 2) state:
00:00:02.836010 eax=00000000 ebx=00000000 ecx=00000000 edx=00000600 esi=00000000 edi=00000000
00:00:02.836012 eip=0000fff0 esp=00000000 ebp=00000000 iopl=0  iopl=0 nv up di pl nz na pe nc
00:00:02.836013 cs={f000 base=00000000ffff0000 limit=0000ffff flags=0000009b} dr0=00000000 dr1=00000000
00:00:02.836016 ds={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr2=00000000 dr3=00000000
00:00:02.836017 es={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr4=00000000 dr5=00000000
00:00:02.836019 fs={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr6=ffff0ff0 dr7=00000400
00:00:02.836021 gs={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr0=60000010 cr2=00000000
00:00:02.836022 ss={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr3=00000000 cr4=00000000
00:00:02.836024 gdtr=0000000000000000:ffff  idtr=0000000000000000:ffff  eflags=00000002
00:00:02.836025 ldtr={0000 base=00000000 limit=0000ffff flags=00000082}
00:00:02.836026 tr  ={0000 base=00000000 limit=0000ffff flags=0000008b}
00:00:02.836027 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.836039 xcr=0000000000000001 xcr1=0000000000000000 xss=0000000000000000 (fXStateMask=0000000000000000)
00:00:02.836042 FCW=037f FSW=0000 FTW=0000 FOP=0000 MXCSR=00001f80 MXCSR_MASK=0000ffff
00:00:02.836044 FPUIP=00000000 CS=0000 Rsrvd1=0000  FPUDP=00000000 DS=0000 Rsvrd2=0000
00:00:02.836047 ST(0)=FPR0={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836052 ST(1)=FPR1={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836055 ST(2)=FPR2={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836059 ST(3)=FPR3={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836062 ST(4)=FPR4={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836064 ST(5)=FPR5={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836066 ST(6)=FPR6={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836068 ST(7)=FPR7={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836069 XMM0 =00000000'00000000'00000000'00000000  XMM1 =00000000'00000000'00000000'00000000
00:00:02.836074 XMM2 =00000000'00000000'00000000'00000000  XMM3 =00000000'00000000'00000000'00000000
00:00:02.836078 XMM4 =00000000'00000000'00000000'00000000  XMM5 =00000000'00000000'00000000'00000000
00:00:02.836082 XMM6 =00000000'00000000'00000000'00000000  XMM7 =00000000'00000000'00000000'00000000
00:00:02.836087 XMM8 =00000000'00000000'00000000'00000000  XMM9 =00000000'00000000'00000000'00000000
00:00:02.836091 XMM10=00000000'00000000'00000000'00000000  XMM11=00000000'00000000'00000000'00000000
00:00:02.836095 XMM12=00000000'00000000'00000000'00000000  XMM13=00000000'00000000'00000000'00000000
00:00:02.836099 XMM14=00000000'00000000'00000000'00000000  XMM15=00000000'00000000'00000000'00000000
00:00:02.836103 EFER         =0000000000000000
00:00:02.836104 PAT          =0007040600070406
00:00:02.836105 STAR         =0000000000000000
00:00:02.836105 CSTAR        =0000000000000000
00:00:02.836106 LSTAR        =0000000000000000
00:00:02.836107 SFMASK       =0000000000000000
00:00:02.836107 KERNELGSBASE =0000000000000000
00:00:02.836110 MTRR_CAP          =0000000000000510
00:00:02.836111 MTRR_DEF_TYPE     =0000000000000c00
00:00:02.836112 MTRR_FIX64K_00000 =0606060606060606
00:00:02.836112 MTRR_FIX16K_80000 =0606060606060606
00:00:02.836113 MTRR_FIX16K_A0000 =0000000000000000
00:00:02.836114 MTRR_FIX4K_C0000  =0505050505050505
00:00:02.836115 MTRR_FIX4K_C8000  =0505050505050505
00:00:02.836116 MTRR_FIX4K_D0000  =0505050505050505
00:00:02.836116 MTRR_FIX4K_D8000  =0505050505050505
00:00:02.836117 MTRR_FIX4K_E0000  =0505050505050505
00:00:02.836118 MTRR_FIX4K_E8000  =0505050505050505
00:00:02.836119 MTRR_FIX4K_F0000  =0505050505050505
00:00:02.836119 MTRR_FIX4K_F8000  =0505050505050505
00:00:02.836127 MTRR_PHYSBASE[ 0] =0000000000000006 First=0000000000000000      0 MB [WB]
00:00:02.836129 MTRR_PHYSMASK[ 0] =0000000f00000800 Last =00000000ffffffff   4095 MB [4096 MB]
00:00:02.836134 MTRR_PHYSBASE[ 1] =00000000e0000000 First=00000000e0000000   3584 MB [UC]
00:00:02.836135 MTRR_PHYSMASK[ 1] =0000000fe0000800 Last =00000000ffffffff   4095 MB [512 MB]
00:00:02.836140 MTRR_PHYSBASE[ 2] =0000000100000006 First=0000000100000000   4096 MB [WB]
00:00:02.836141 MTRR_PHYSMASK[ 2] =0000000fe0000800 Last =000000011fffffff   4607 MB [512 MB]
00:00:02.836146 MTRR_PHYSBASE[ 3] =0000000000000000
00:00:02.836146 MTRR_PHYSMASK[ 3] =0000000000000000
00:00:02.836148 MTRR_PHYSBASE[ 4] =0000000000000000
00:00:02.836149 MTRR_PHYSMASK[ 4] =0000000000000000
00:00:02.836150 MTRR_PHYSBASE[ 5] =0000000000000000
00:00:02.836151 MTRR_PHYSMASK[ 5] =0000000000000000
00:00:02.836152 MTRR_PHYSBASE[ 6] =0000000000000000
00:00:02.836153 MTRR_PHYSMASK[ 6] =0000000000000000
00:00:02.836154 MTRR_PHYSBASE[ 7] =0000000000000000
00:00:02.836155 MTRR_PHYSMASK[ 7] =0000000000000000
00:00:02.836156 MTRR_PHYSBASE[ 8] =0000000000000000
00:00:02.836156 MTRR_PHYSMASK[ 8] =0000000000000000
00:00:02.836157 MTRR_PHYSBASE[ 9] =0000000000000000
00:00:02.836157 MTRR_PHYSMASK[ 9] =0000000000000000
00:00:02.836158 MTRR_PHYSBASE[10] =0000000000000000
00:00:02.836158 MTRR_PHYSMASK[10] =0000000000000000
00:00:02.836159 MTRR_PHYSBASE[11] =0000000000000000
00:00:02.836160 MTRR_PHYSMASK[11] =0000000000000000
00:00:02.836160 MTRR_PHYSBASE[12] =0000000000000000
00:00:02.836161 MTRR_PHYSMASK[12] =0000000000000000
00:00:02.836161 MTRR_PHYSBASE[13] =0000000000000000
00:00:02.836162 MTRR_PHYSMASK[13] =0000000000000000
00:00:02.836163 MTRR_PHYSBASE[14] =0000000000000000
00:00:02.836163 MTRR_PHYSMASK[14] =0000000000000000
00:00:02.836164 MTRR_PHYSBASE[15] =0000000000000000
00:00:02.836164 MTRR_PHYSMASK[15] =0000000000000000
00:00:02.836198 Guest CPUM (VCPU 3) state:
00:00:02.836201 eax=00000000 ebx=00000000 ecx=00000000 edx=00000600 esi=00000000 edi=00000000
00:00:02.836203 eip=0000fff0 esp=00000000 ebp=00000000 iopl=0  iopl=0 nv up di pl nz na pe nc
00:00:02.836204 cs={f000 base=00000000ffff0000 limit=0000ffff flags=0000009b} dr0=00000000 dr1=00000000
00:00:02.836207 ds={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr2=00000000 dr3=00000000
00:00:02.836209 es={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr4=00000000 dr5=00000000
00:00:02.836210 fs={0000 base=0000000000000000 limit=0000ffff flags=00000093} dr6=ffff0ff0 dr7=00000400
00:00:02.836212 gs={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr0=60000010 cr2=00000000
00:00:02.836214 ss={0000 base=0000000000000000 limit=0000ffff flags=00000093} cr3=00000000 cr4=00000000
00:00:02.836216 gdtr=0000000000000000:ffff  idtr=0000000000000000:ffff  eflags=00000002
00:00:02.836217 ldtr={0000 base=00000000 limit=0000ffff flags=00000082}
00:00:02.836218 tr  ={0000 base=00000000 limit=0000ffff flags=0000008b}
00:00:02.836219 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.836231 xcr=0000000000000001 xcr1=0000000000000000 xss=0000000000000000 (fXStateMask=0000000000000000)
00:00:02.836234 FCW=037f FSW=0000 FTW=0000 FOP=0000 MXCSR=00001f80 MXCSR_MASK=0000ffff
00:00:02.836235 FPUIP=00000000 CS=0000 Rsrvd1=0000  FPUDP=00000000 DS=0000 Rsvrd2=0000
00:00:02.836239 ST(0)=FPR0={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836244 ST(1)=FPR1={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836247 ST(2)=FPR2={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836250 ST(3)=FPR3={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836253 ST(4)=FPR4={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836256 ST(5)=FPR5={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836259 ST(6)=FPR6={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836262 ST(7)=FPR7={0000'00000000'00000000} t0 +0.0000000000000000000000 * 2 ^ -16383 (*)
00:00:02.836265 XMM0 =00000000'00000000'00000000'00000000  XMM1 =00000000'00000000'00000000'00000000
00:00:02.836270 XMM2 =00000000'00000000'00000000'00000000  XMM3 =00000000'00000000'00000000'00000000
00:00:02.836274 XMM4 =00000000'00000000'00000000'00000000  XMM5 =00000000'00000000'00000000'00000000
00:00:02.836278 XMM6 =00000000'00000000'00000000'00000000  XMM7 =00000000'00000000'00000000'00000000
00:00:02.836283 XMM8 =00000000'00000000'00000000'00000000  XMM9 =00000000'00000000'00000000'00000000
00:00:02.836286 XMM10=00000000'00000000'00000000'00000000  XMM11=00000000'00000000'00000000'00000000
00:00:02.836289 XMM12=00000000'00000000'00000000'00000000  XMM13=00000000'00000000'00000000'00000000
00:00:02.836291 XMM14=00000000'00000000'00000000'00000000  XMM15=00000000'00000000'00000000'00000000
00:00:02.836294 EFER         =0000000000000000
00:00:02.836295 PAT          =0007040600070406
00:00:02.836296 STAR         =0000000000000000
00:00:02.836297 CSTAR        =0000000000000000
00:00:02.836297 LSTAR        =0000000000000000
00:00:02.836298 SFMASK       =0000000000000000
00:00:02.836298 KERNELGSBASE =0000000000000000
00:00:02.836301 MTRR_CAP          =0000000000000510
00:00:02.836302 MTRR_DEF_TYPE     =0000000000000c00
00:00:02.836302 MTRR_FIX64K_00000 =0606060606060606
00:00:02.836303 MTRR_FIX16K_80000 =0606060606060606
00:00:02.836304 MTRR_FIX16K_A0000 =0000000000000000
00:00:02.836305 MTRR_FIX4K_C0000  =0505050505050505
00:00:02.836305 MTRR_FIX4K_C8000  =0505050505050505
00:00:02.836306 MTRR_FIX4K_D0000  =0505050505050505
00:00:02.836307 MTRR_FIX4K_D8000  =0505050505050505
00:00:02.836317 MTRR_FIX4K_E0000  =0505050505050505
00:00:02.836318 MTRR_FIX4K_E8000  =0505050505050505
00:00:02.836318 MTRR_FIX4K_F0000  =0505050505050505
00:00:02.836319 MTRR_FIX4K_F8000  =0505050505050505
00:00:02.836326 MTRR_PHYSBASE[ 0] =0000000000000006 First=0000000000000000      0 MB [WB]
00:00:02.836328 MTRR_PHYSMASK[ 0] =0000000f00000800 Last =00000000ffffffff   4095 MB [4096 MB]
00:00:02.836333 MTRR_PHYSBASE[ 1] =00000000e0000000 First=00000000e0000000   3584 MB [UC]
00:00:02.836335 MTRR_PHYSMASK[ 1] =0000000fe0000800 Last =00000000ffffffff   4095 MB [512 MB]
00:00:02.836339 MTRR_PHYSBASE[ 2] =0000000100000006 First=0000000100000000   4096 MB [WB]
00:00:02.836340 MTRR_PHYSMASK[ 2] =0000000fe0000800 Last =000000011fffffff   4607 MB [512 MB]
00:00:02.836345 MTRR_PHYSBASE[ 3] =0000000000000000
00:00:02.836346 MTRR_PHYSMASK[ 3] =0000000000000000
00:00:02.836347 MTRR_PHYSBASE[ 4] =0000000000000000
00:00:02.836348 MTRR_PHYSMASK[ 4] =0000000000000000
00:00:02.836349 MTRR_PHYSBASE[ 5] =0000000000000000
00:00:02.836350 MTRR_PHYSMASK[ 5] =0000000000000000
00:00:02.836351 MTRR_PHYSBASE[ 6] =0000000000000000
00:00:02.836351 MTRR_PHYSMASK[ 6] =0000000000000000
00:00:02.836352 MTRR_PHYSBASE[ 7] =0000000000000000
00:00:02.836353 MTRR_PHYSMASK[ 7] =0000000000000000
00:00:02.836353 MTRR_PHYSBASE[ 8] =0000000000000000
00:00:02.836354 MTRR_PHYSMASK[ 8] =0000000000000000
00:00:02.836355 MTRR_PHYSBASE[ 9] =0000000000000000
00:00:02.836355 MTRR_PHYSMASK[ 9] =0000000000000000
00:00:02.836356 MTRR_PHYSBASE[10] =0000000000000000
00:00:02.836356 MTRR_PHYSMASK[10] =0000000000000000
00:00:02.836357 MTRR_PHYSBASE[11] =0000000000000000
00:00:02.836357 MTRR_PHYSMASK[11] =0000000000000000
00:00:02.836358 MTRR_PHYSBASE[12] =0000000000000000
00:00:02.836359 MTRR_PHYSMASK[12] =0000000000000000
00:00:02.836359 MTRR_PHYSBASE[13] =0000000000000000
00:00:02.836360 MTRR_PHYSMASK[13] =0000000000000000
00:00:02.836361 MTRR_PHYSBASE[14] =0000000000000000
00:00:02.836362 MTRR_PHYSMASK[14] =0000000000000000
00:00:02.836363 MTRR_PHYSBASE[15] =0000000000000000
00:00:02.836364 MTRR_PHYSMASK[15] =0000000000000000
00:00:02.836394 !!
00:00:02.836394 !! {cpumguesthwvirt, verbose}
00:00:02.836395 !!
00:00:02.836399 VCPU[0] hardware virtualization state:
00:00:02.836399 fSavedInhibit                = 0x0
00:00:02.836401 In nested-guest hwvirt mode  = false
00:00:02.836402 Hwvirt state disabled.
00:00:02.836403 VCPU[1] hardware virtualization state:
00:00:02.836405 fSavedInhibit                = 0x0
00:00:02.836406 In nested-guest hwvirt mode  = false
00:00:02.836407 Hwvirt state disabled.
00:00:02.836414 VCPU[2] hardware virtualization state:
00:00:02.836416 fSavedInhibit                = 0x0
00:00:02.836417 In nested-guest hwvirt mode  = false
00:00:02.836418 Hwvirt state disabled.
00:00:02.836428 VCPU[3] hardware virtualization state:
00:00:02.836429 fSavedInhibit                = 0x0
00:00:02.836430 In nested-guest hwvirt mode  = false
00:00:02.836432 Hwvirt state disabled.
00:00:02.836439 !!
00:00:02.836439 !! {cpumguestinstr, verbose}
00:00:02.836440 !!
00:00:02.836465
00:00:02.836465 CPUM0: 0008:001010f4 0f 22 e0                mov cr4, eax
00:00:02.836466
00:00:02.836525
00:00:02.836525 CPUM1: f000:fff0 ea 5b e0 00 f0          jmp far 0f000h:0e05bh
00:00:02.836526
00:00:02.836579
00:00:02.836579 CPUM2: f000:fff0 ea 5b e0 00 f0          jmp far 0f000h:0e05bh
00:00:02.836580
00:00:02.836598
00:00:02.836598 CPUM3: f000:fff0 ea 5b e0 00 f0          jmp far 0f000h:0e05bh
00:00:02.836599
00:00:02.836631 !!
00:00:02.836632 !! {cpumhyper, verbose}
00:00:02.836632 !!
00:00:02.836635 Hypervisor CPUM state:
00:00:02.836636 .dr0=0000000000000000 .dr1=0000000000000000 .dr2=0000000000000000 .dr3=0000000000000000
00:00:02.836637 .dr4=0000000000000000 .dr5=0000000000000000 .dr6=0000000000000000 .dr7=0000000000000000
00:00:02.836641 Hypervisor CPUM state:
00:00:02.836642 .dr0=0000000000000000 .dr1=0000000000000000 .dr2=0000000000000000 .dr3=0000000000000000
00:00:02.836643 .dr4=0000000000000000 .dr5=0000000000000000 .dr6=0000000000000000 .dr7=0000000000000000
00:00:02.836655 Hypervisor CPUM state:
00:00:02.836656 .dr0=0000000000000000 .dr1=0000000000000000 .dr2=0000000000000000 .dr3=0000000000000000
00:00:02.836658 .dr4=0000000000000000 .dr5=0000000000000000 .dr6=0000000000000000 .dr7=0000000000000000
00:00:02.836672 Hypervisor CPUM state:
00:00:02.836673 .dr0=0000000000000000 .dr1=0000000000000000 .dr2=0000000000000000 .dr3=0000000000000000
00:00:02.836675 .dr4=0000000000000000 .dr5=0000000000000000 .dr6=0000000000000000 .dr7=0000000000000000
00:00:02.836696 !!
00:00:02.836697 !! {cpumhost, verbose}
00:00:02.836698 !!
00:00:02.836701 Host CPUM state:
00:00:02.836702 rax=xxxxxxxxxxxxxxxx rbx=0000000000000000 rcx=xxxxxxxxxxxxxxxx
00:00:02.836702 rdx=xxxxxxxxxxxxxxxx rsi=0000000000000000 rdi=0000000000000000
00:00:02.836703 rip=xxxxxxxxxxxxxxxx rsp=0000000000000000 rbp=0000000000000000
00:00:02.836704  r8=xxxxxxxxxxxxxxxx  r9=xxxxxxxxxxxxxxxx r10=0000000000000000
00:00:02.836704 r11=0000000000000000 r12=0000000000000000 r13=0000000000000000
00:00:02.836705 r14=0000000000000000 r15=0000000000000000
00:00:02.836706 iopl=0   iopl=0 nv up di pl nz na pe nc
00:00:02.836707 cs=0000  ds=0000  es=0000  fs=0000  gs=0000                   eflags=00000000
00:00:02.836708 cr0=0000000000000000 cr2=xxxxxxxxxxxxxxxx cr3=0000000000000000
00:00:02.836709 cr4=0000000000000000 ldtr=0000 tr=0000
00:00:02.836709 dr[0]=0000000000000000 dr[1]=0000000000000000 dr[2]=0000000000000000
00:00:02.836710 dr[3]=0000000000000000 dr[6]=0000000000000000 dr[7]=0000000000000000
00:00:02.836711 gdtr=0000000000000000:0000  idtr=0000000000000000:0000
00:00:02.836712 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.836712 FSbase=0000000000000000 GSbase=0000000000000000 efer=00000000
00:00:02.836721 Host CPUM state:
00:00:02.836722 rax=xxxxxxxxxxxxxxxx rbx=0000000000000000 rcx=xxxxxxxxxxxxxxxx
00:00:02.836723 rdx=xxxxxxxxxxxxxxxx rsi=0000000000000000 rdi=0000000000000000
00:00:02.836724 rip=xxxxxxxxxxxxxxxx rsp=0000000000000000 rbp=0000000000000000
00:00:02.836725  r8=xxxxxxxxxxxxxxxx  r9=xxxxxxxxxxxxxxxx r10=0000000000000000
00:00:02.836725 r11=0000000000000000 r12=0000000000000000 r13=0000000000000000
00:00:02.836726 r14=0000000000000000 r15=0000000000000000
00:00:02.836727 iopl=0   iopl=0 nv up di pl nz na pe nc
00:00:02.836728 cs=0000  ds=0000  es=0000  fs=0000  gs=0000                   eflags=00000000
00:00:02.836729 cr0=0000000000000000 cr2=xxxxxxxxxxxxxxxx cr3=0000000000000000
00:00:02.836730 cr4=0000000000000000 ldtr=0000 tr=0000
00:00:02.836731 dr[0]=0000000000000000 dr[1]=0000000000000000 dr[2]=0000000000000000
00:00:02.836732 dr[3]=0000000000000000 dr[6]=0000000000000000 dr[7]=0000000000000000
00:00:02.836733 gdtr=0000000000000000:0000  idtr=0000000000000000:0000
00:00:02.836734 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.836734 FSbase=0000000000000000 GSbase=0000000000000000 efer=00000000
00:00:02.836769 Host CPUM state:
00:00:02.836771 rax=xxxxxxxxxxxxxxxx rbx=0000000000000000 rcx=xxxxxxxxxxxxxxxx
00:00:02.836772 rdx=xxxxxxxxxxxxxxxx rsi=0000000000000000 rdi=0000000000000000
00:00:02.836772 rip=xxxxxxxxxxxxxxxx rsp=0000000000000000 rbp=0000000000000000
00:00:02.836773  r8=xxxxxxxxxxxxxxxx  r9=xxxxxxxxxxxxxxxx r10=0000000000000000
00:00:02.836774 r11=0000000000000000 r12=0000000000000000 r13=0000000000000000
00:00:02.836775 r14=0000000000000000 r15=0000000000000000
00:00:02.836775 iopl=0   iopl=0 nv up di pl nz na pe nc
00:00:02.836776 cs=0000  ds=0000  es=0000  fs=0000  gs=0000                   eflags=00000000
00:00:02.836778 cr0=0000000000000000 cr2=xxxxxxxxxxxxxxxx cr3=0000000000000000
00:00:02.836779 cr4=0000000000000000 ldtr=0000 tr=0000
00:00:02.836779 dr[0]=0000000000000000 dr[1]=0000000000000000 dr[2]=0000000000000000
00:00:02.836780 dr[3]=0000000000000000 dr[6]=0000000000000000 dr[7]=0000000000000000
00:00:02.836781 gdtr=0000000000000000:0000  idtr=0000000000000000:0000
00:00:02.836782 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.836783 FSbase=0000000000000000 GSbase=0000000000000000 efer=00000000
00:00:02.836799 Host CPUM state:
00:00:02.836801 rax=xxxxxxxxxxxxxxxx rbx=0000000000000000 rcx=xxxxxxxxxxxxxxxx
00:00:02.836801 rdx=xxxxxxxxxxxxxxxx rsi=0000000000000000 rdi=0000000000000000
00:00:02.836802 rip=xxxxxxxxxxxxxxxx rsp=0000000000000000 rbp=0000000000000000
00:00:02.836802  r8=xxxxxxxxxxxxxxxx  r9=xxxxxxxxxxxxxxxx r10=0000000000000000
00:00:02.836803 r11=0000000000000000 r12=0000000000000000 r13=0000000000000000
00:00:02.836803 r14=0000000000000000 r15=0000000000000000
00:00:02.836804 iopl=0   iopl=0 nv up di pl nz na pe nc
00:00:02.836804 cs=0000  ds=0000  es=0000  fs=0000  gs=0000                   eflags=00000000
00:00:02.836805 cr0=0000000000000000 cr2=xxxxxxxxxxxxxxxx cr3=0000000000000000
00:00:02.836805 cr4=0000000000000000 ldtr=0000 tr=0000
00:00:02.836806 dr[0]=0000000000000000 dr[1]=0000000000000000 dr[2]=0000000000000000
00:00:02.836807 dr[3]=0000000000000000 dr[6]=0000000000000000 dr[7]=0000000000000000
00:00:02.836808 gdtr=0000000000000000:0000  idtr=0000000000000000:0000
00:00:02.836809 SysEnter={cs=0000 eip=00000000 esp=00000000}
00:00:02.836809 FSbase=0000000000000000 GSbase=0000000000000000 efer=00000000
00:00:02.836827 !!
00:00:02.836828 !! {mode, all}
00:00:02.836829 !!
00:00:02.836834 Guest paging mode (VCPU #0):  Protected (changed 778 times), A20 enabled (changed 2 times)
00:00:02.836837 Guest SLAT mode (VCPU #0): Direct
00:00:02.836838 Shadow paging mode (VCPU #0): EPT
00:00:02.836839 Host paging mode:             AMD64+G+NX
00:00:02.836841 Guest paging mode (VCPU #1):  Real (changed 1 times), A20 enabled (changed 0 times)
00:00:02.836845 Guest SLAT mode (VCPU #1): Direct
00:00:02.836846 Shadow paging mode (VCPU #1): EPT
00:00:02.836847 Host paging mode:             AMD64+G+NX
00:00:02.836879 Guest paging mode (VCPU #2):  Real (changed 1 times), A20 enabled (changed 0 times)
00:00:02.836883 Guest SLAT mode (VCPU #2): Direct
00:00:02.836883 Shadow paging mode (VCPU #2): EPT
00:00:02.836885 Host paging mode:             AMD64+G+NX
00:00:02.836916 Guest paging mode (VCPU #3):  Real (changed 1 times), A20 enabled (changed 0 times)
00:00:02.836919 Guest SLAT mode (VCPU #3): Direct
00:00:02.836920 Shadow paging mode (VCPU #3): EPT
00:00:02.836921 Host paging mode:             AMD64+G+NX
00:00:02.836930 !!
00:00:02.836931 !! {cpuid, verbose}
00:00:02.836931 !!
00:00:02.837171          Raw Standard CPUID Leaves
00:00:02.837171      Leaf/sub-leaf  eax      ebx      ecx      edx
00:00:02.837173 Gst: 00000000/0000  0000000d 756e6547 6c65746e 49656e69
00:00:02.837175 Hst:                0000000d 756e6547 6c65746e 49656e69
00:00:02.837177 Gst: 00000001/0000  000306a9 00040800 769a2203 178bfbbf
00:00:02.837179 Hst:                000306a9 05100800 7fbae3ff bfebfbff
00:00:02.837180 Gst: 00000002/0000  76035a01 00f0b2ff 00000000 00ca0000
00:00:02.837182 Hst:                76035a01 00f0b2ff 00000000 00ca0000
00:00:02.837184 Gst: 00000003/0000  00000000 00000000 00000000 00000000
00:00:02.837185 Hst:                00000000 00000000 00000000 00000000
00:00:02.837186 Gst: 00000004/0000  0c000121 01c0003f 0000003f 00000000
00:00:02.837188 Hst:                1c004121 01c0003f 0000003f 00000000
00:00:02.837190 Gst: 00000004/0001  0c000122 01c0003f 0000003f 00000000
00:00:02.837191 Hst:                1c004122 01c0003f 0000003f 00000000
00:00:02.837192 Gst: 00000004/0002  0c000143 01c0003f 000001ff 00000000
00:00:02.837194 Hst:                1c004143 01c0003f 000001ff 00000000
00:00:02.837195 Gst: 00000004/0003  0c000163 03c0003f 00001fff 00000006
00:00:02.837197 Hst:                1c03c163 03c0003f 00001fff 00000006
00:00:02.837198 Gst: 00000004/0004  0c000000 00000000 00000000 00000000
00:00:02.837200 Hst:                00000000 00000000 00000000 00000000
00:00:02.837201 Gst: 00000005/0000  00000000 00000000 00000000 00000000
00:00:02.837202 Hst:                00000040 00000040 00000003 00001120
00:00:02.837204 Gst: 00000006/0000  00000004 00000000 00000000 00000000
00:00:02.837205 Hst:                00000077 00000002 00000009 00000000
00:00:02.837206 Gst: 00000007/0000  00000000 00000001 00000000 10000400
00:00:02.837208 Hst:                00000000 00000281 00000000 9c000400
00:00:02.837209 Gst: 00000007/0001  00000000 00000000 00000000 00000000
00:00:02.837210 Hst:                00000000 00000000 00000000 00000000
00:00:02.837211 Gst: 00000007/0002  00000000 00000000 00000000 00000000
00:00:02.837213 Hst:                00000000 00000000 00000000 00000000
00:00:02.837214 Gst: 00000008/0000  00000000 00000000 00000000 00000000
00:00:02.837215 Hst:                00000000 00000000 00000000 00000000
00:00:02.837216 Gst: 00000009/0000  00000000 00000000 00000000 00000000
00:00:02.837218 Hst:                00000000 00000000 00000000 00000000
00:00:02.837219 Gst: 0000000a/0000  00000000 00000000 00000000 00000000
00:00:02.837220 Hst:                07300403 00000000 00000000 00000603
00:00:02.837221 Gst: 0000000b/0000  00000000 00000001 00000100 00000000
00:00:02.837223 Hst:                00000001 00000002 00000100 00000005
00:00:02.837224 Gst: 0000000b/0001  00000002 00000004 00000201 00000000
00:00:02.837225 Hst:                00000004 00000008 00000201 00000005
00:00:02.837226 Gst: 0000000b/0002  00000000 00000000 00000002 00000000
00:00:02.837227 Hst:                00000000 00000000 00000002 00000005
00:00:02.837229 Gst: 0000000c/0000  00000000 00000000 00000000 00000000
00:00:02.837230 Hst:                00000000 00000000 00000000 00000000
00:00:02.837231 Gst: 0000000d/0000  00000007 00000340 00000340 00000000
00:00:02.837232 Hst:                00000007 00000340 00000340 00000000
00:00:02.837233 Gst: 0000000d/0001  00000000 00000000 00000000 00000000
00:00:02.837235 Hst:                00000001 00000000 00000000 00000000
00:00:02.837236 Gst: 0000000d/0002  00000100 00000240 00000000 00000000
00:00:02.837237 Hst:                00000100 00000240 00000000 00000000
00:00:02.837239 Gst: 0000000d/0003  00000000 00000000 00000000 00000000
00:00:02.837240 Hst:                00000000 00000000 00000000 00000000
00:00:02.837241                                Name: GenuineIntel
00:00:02.837242                            Supports: 0x00000000-0x0000000d
00:00:02.837245                              Family:  6 	Extended: 0 	Effective: 6
00:00:02.837246                               Model: 10 	Extended: 3 	Effective: 58
00:00:02.837247                            Stepping: 9
00:00:02.837248                                Type: 0 (primary)
00:00:02.837250                             APIC ID: 0x00
00:00:02.837250                        Logical CPUs: 4
00:00:02.837251                        CLFLUSH Size: 8
00:00:02.837252                            Brand ID: 0x00
00:00:02.837257 Features
00:00:02.837257   Mnemonic - Description                                  = Guest (Host)
00:00:02.837260   FPU - x87 FPU on Chip                                   = 1 (1)
00:00:02.837262   VME - Virtual 8086 Mode Enhancements                    = 1 (1)
00:00:02.837264   DE - Debugging extensions                               = 1 (1)
00:00:02.837265   PSE - Page Size Extension                               = 1 (1)
00:00:02.837267   TSC - Time Stamp Counter                                = 1 (1)
00:00:02.837269   MSR - Model Specific Registers                          = 1 (1)
00:00:02.837271   PAE - Physical Address Extension                        = 0 (1)
00:00:02.837272   MCE - Machine Check Exception                           = 1 (1)
00:00:02.837274   CX8 - CMPXCHG8B instruction                             = 1 (1)
00:00:02.837276   APIC - APIC On-Chip                                     = 1 (1)
00:00:02.837277   SEP - SYSENTER and SYSEXIT Present                      = 1 (1)
00:00:02.837279   MTRR - Memory Type Range Registers                      = 1 (1)
00:00:02.837281   PGE - PTE Global Bit                                    = 1 (1)
00:00:02.837282   MCA - Machine Check Architecture                        = 1 (1)
00:00:02.837284   CMOV - Conditional Move instructions                    = 1 (1)
00:00:02.837285   PAT - Page Attribute Table                              = 1 (1)
00:00:02.837287   PSE-36 - 36-bit Page Size Extension                     = 1 (1)
00:00:02.837289   PSN - Processor Serial Number                           = 0 (0)
00:00:02.837290   CLFSH - CLFLUSH instruction                             = 1 (1)
00:00:02.837292   DS - Debug Store                                        = 0 (1)
00:00:02.837294   ACPI - Thermal Mon. & Soft. Clock Ctrl.                 = 0 (1)
00:00:02.837295   MMX - Intel MMX Technology                              = 1 (1)
00:00:02.837297   FXSR - FXSAVE and FXRSTOR instructions                  = 1 (1)
00:00:02.837298   SSE - SSE support                                       = 1 (1)
00:00:02.837300   SSE2 - SSE2 support                                     = 1 (1)
00:00:02.837302   SS - Self Snoop                                         = 0 (1)
00:00:02.837304   HTT - Hyper-Threading Technology                        = 1 (1)
00:00:02.837305   TM - Therm. Monitor                                     = 0 (1)
00:00:02.837315   PBE - Pending Break Enabled                             = 0 (1)
00:00:02.837317   SSE3 - SSE3 support                                     = 1 (1)
00:00:02.837319   PCLMUL - PCLMULQDQ support (for AES-GCM)                = 1 (1)
00:00:02.837320   DTES64 - DS Area 64-bit Layout                          = 0 (1)
00:00:02.837321   MONITOR - MONITOR/MWAIT instructions                    = 0 (1)
00:00:02.837323   CPL-DS - CPL Qualified Debug Store                      = 0 (1)
00:00:02.837324   VMX - Virtual Machine Extensions                        = 0 (1)
00:00:02.837326   SMX - Safer Mode Extensions                             = 0 (1)
00:00:02.837328   EST - Enhanced SpeedStep Technology                     = 0 (1)
00:00:02.837329   TM2 - Terminal Monitor 2                                = 0 (1)
00:00:02.837331   SSSE3 - Supplemental Streaming SIMD Extensions 3        = 1 (1)
00:00:02.837332   CNTX-ID - L1 Context ID                                 = 0 (0)
00:00:02.837334   SDBG - Silicon Debug interface                          = 0 (0)
00:00:02.837335   FMA - Fused Multiply Add extensions                     = 0 (0)
00:00:02.837337   CX16 - CMPXCHG16B instruction                           = 1 (1)
00:00:02.837338   TPRUPDATE - xTPR Update Control                         = 0 (1)
00:00:02.837340   PDCM - Perf/Debug Capability MSR                        = 0 (1)
00:00:02.837342   PCID - Process Context Identifiers                      = 1 (1)
00:00:02.837343   DCA - Direct Cache Access                               = 0 (0)
00:00:02.837345   SSE4_1 - SSE4_1 support                                 = 1 (1)
00:00:02.837347   SSE4_2 - SSE4_2 support                                 = 1 (1)
00:00:02.837348   X2APIC - x2APIC support                                 = 0 (1)
00:00:02.837350   MOVBE - MOVBE instruction                               = 0 (0)
00:00:02.837352   POPCNT - POPCNT instruction                             = 1 (1)
00:00:02.837353   TSCDEADL - Time Stamp Counter Deadline                  = 0 (1)
00:00:02.837355   AES - AES instructions                                  = 1 (1)
00:00:02.837356   XSAVE - XSAVE instruction                               = 1 (1)
00:00:02.837358   OSXSAVE - OSXSAVE instruction                           = 0 (1)
00:00:02.837360   AVX - AVX support                                       = 1 (1)
00:00:02.837362   F16C - 16-bit floating point conversion instructions    = 1 (1)
00:00:02.837363   RDRAND - RDRAND instruction                             = 1 (1)
00:00:02.837364   HVP - Hypervisor Present (we're a guest)                = 0 (0)
00:00:02.837366 Structured Extended Feature Flags Enumeration (leaf 7):
00:00:02.837367 Sub-leaf 0
00:00:02.837367   Mnemonic - Description                                  = Guest (Host)
00:00:02.837369   FSGSBASE - RDFSBASE/RDGSBASE/WRFSBASE/WRGSBASE instr.   = 1 (1)
00:00:02.837370   TSCADJUST - Supports MSR_IA32_TSC_ADJUST                = 0 (0)
00:00:02.837371   SGX - Supports Software Guard Extensions                = 0 (0)
00:00:02.837373   BMI1 - Advanced Bit Manipulation extension 1            = 0 (0)
00:00:02.837374   HLE - Hardware Lock Elision                             = 0 (0)
00:00:02.837376   AVX2 - Advanced Vector Extensions 2                     = 0 (0)
00:00:02.837378   FDP_EXCPTN_ONLY - FPU DP only updated on exceptions     = 0 (0)
00:00:02.837379   SMEP - Supervisor Mode Execution Prevention             = 0 (1)
00:00:02.837380   BMI2 - Advanced Bit Manipulation extension 2            = 0 (0)
00:00:02.837381   ERMS - Enhanced REP MOVSB/STOSB instructions            = 0 (1)
00:00:02.837383   INVPCID - INVPCID instruction                           = 0 (0)
00:00:02.837384   RTM - Restricted Transactional Memory                   = 0 (0)
00:00:02.837386   PQM - Platform Quality of Service Monitoring            = 0 (0)
00:00:02.837387   DEPFPU_CS_DS - Deprecates FPU CS, FPU DS values if set  = 0 (0)
00:00:02.837388   MPE - Intel Memory Protection Extensions                = 0 (0)
00:00:02.837389   PQE - Platform Quality of Service Enforcement           = 0 (0)
00:00:02.837391   AVX512F - AVX512 Foundation instructions                = 0 (0)
00:00:02.837392   AVX512DQ - Supports the AVX512DQ instructions           = 0 (0)
00:00:02.837393   RDSEED - RDSEED instruction                             = 0 (0)
00:00:02.837395   ADX - ADCX/ADOX instructions                            = 0 (0)
00:00:02.837396   SMAP - Supervisor Mode Access Prevention                = 0 (0)
00:00:02.837398   AVX512_IFMA - Supports the AVX512_IFMA instructions     = 0 (0)
00:00:02.837399   CLFLUSHOPT - CLFLUSHOPT (Cache Line Flush) instruction  = 0 (0)
00:00:02.837400   CLWB - CLWB instruction                                 = 0 (0)
00:00:02.837402   INTEL_PT - Intel Processor Trace                        = 0 (0)
00:00:02.837403   AVX512PF - AVX512 Prefetch instructions                 = 0 (0)
00:00:02.837405   AVX512ER - AVX512 Exponential & Reciprocal instructions = 0 (0)
00:00:02.837406   AVX512CD - AVX512 Conflict Detection instructions       = 0 (0)
00:00:02.837407   SHA - Secure Hash Algorithm extensions                  = 0 (0)
00:00:02.837408   AVX512BW - Supports the AVX512BW instructions           = 0 (0)
00:00:02.837409   AVX512VL - Supports the AVX512VL instructions           = 0 (0)
00:00:02.837411   PREFETCHWT1 - PREFETCHWT1 instruction                   = 0 (0)
00:00:02.837412   AVX512_VBMI - Supports the AVX512_VBMI instructions     = 0 (0)
00:00:02.837413   UMIP - User mode insturction prevention                 = 0 (0)
00:00:02.837415   PKU - Protection Key for Usermode pages                 = 0 (0)
00:00:02.837416   OSPKE - CR4.PKU mirror                                  = 0 (0)
00:00:02.837418   WAITPKG - TPAUSE, UMONITOR & UMWAIT support             = 0 (0)
00:00:02.837419   AVX512_VBMI2 - Supports the AVX512_VBMI2 instructions   = 0 (0)
00:00:02.837420   CET_SS - CET shadow stack support                       = 0 (0)
00:00:02.837422   GFNI - Supports the GFNI instruction set                = 0 (0)
00:00:02.837423   VAES - Supports the VEX encoded AES instruction set     = 0 (0)
00:00:02.837424   VPCLMULQDQ - Supports the VPCLMULQDQ instruction        = 0 (0)
00:00:02.837426   AVX512_VNNI - Supports the AVX512_VNNI instructions     = 0 (0)
00:00:02.837427   AVX512_BITALG - Supports the AVX512_BITALG instructions = 0 (0)
00:00:02.837428   TME_EN - Supports 4 IA32_TME_ MSRs                      = 0 (0)
00:00:02.837429   AVX512_VPOPCNTDQ - Supports the AVX512_VPOPCNTDQ instructions = 0 (0)
00:00:02.837430   LA57 - 57-bit linear addresses                          = 0 (0)
00:00:02.837432   MAWAU - Value used by BNDLDX & BNDSTX                   = 0x0 (0x0)
00:00:02.837434   RDPID - Read processor ID support                       = 0 (0)
00:00:02.837435   KEY_LOCKER - Supports Key Locker                        = 0 (0)
00:00:02.837437   BUS_LOCK_DETECT - Supports OS bus-lock detection        = 0 (0)
00:00:02.837438   CLDEMOTE - Supports cache line demote                   = 0 (0)
00:00:02.837439   MOVDIRI - Supports the MOVDIRI instruction              = 0 (0)
00:00:02.837441   MOVDIRI64B - Supports the MOVDIRI64B instruction        = 0 (0)
00:00:02.837442   ENQCMD - Supports the Eqnqueue Stores                   = 0 (0)
00:00:02.837443   SGX_LC - Supports SGX Launch Configuration              = 0 (0)
00:00:02.837445   PKS - Supports protection keys for supervisor pages     = 0 (0)
00:00:02.837446   SGX_KEYS - Supports Attestation Service for Intel SGX   = 0 (0)
00:00:02.837447   AVX512_4VNNIW - Supports the AVX512_4VNNIW instructions = 0 (0)
00:00:02.837448   AVX512_4FMAPS - Supports the AVX512_4FMAPS instructions = 0 (0)
00:00:02.837449   FAST_SHORT_REP_MOVSB - Supports fast short REP MOVSB    = 0 (0)
00:00:02.837450   UINTR - Supports user interrupts                        = 0 (0)
00:00:02.837451   AVX512_VP2INTERSECT - Supports the AVX512_VP2INTERSECT instr. = 0 (0)
00:00:02.837452   MCU_OPT_CTRL - Supports IA32_MCU_OPT_CTRL               = 0 (0)
00:00:02.837454   MD_CLEAR - Supports MDS related buffer clearing         = 1 (1)
00:00:02.837455   RTM_ALWAYS_ABORT - XBEGIN always aborts and does fallback = 0 (0)
00:00:02.837456   RTM_FORCE_ABORT - Supports IA32_TSX_FORCE_ABORT         = 0 (0)
00:00:02.837457   SERIALIZE - Supports the SERIALIZE instruction          = 0 (0)
00:00:02.837458   HYBRID - Identifiers the CPU as a hybrid part           = 0 (0)
00:00:02.837460   TSXLDTRK - Supports susp/resume of TSX ld addr tracking = 0 (0)
00:00:02.837461   PCONFIG - Supports the PCONFIG instruction              = 0 (0)
00:00:02.837462   ARCH_LBRS - Supports architectural LBRs                 = 0 (0)
00:00:02.837463   CET_IBT - Supports indirect branch tracking w/ CET      = 0 (0)
00:00:02.837465   AMX_BF16 - Supports tile comp. ops on bfloat16 number   = 0 (0)
00:00:02.837466   AVX512_FP16 - Supports the FP16 data type with AVX512   = 0 (0)
00:00:02.837467   AMX_TILE - Supports the tile architecture               = 0 (0)
00:00:02.837468   AMX_INT8 - Supports tile comp. ops on 8-bit integers    = 0 (0)
00:00:02.837469   IBRS_IBPB - IA32_SPEC_CTRL.IBRS and IA32_PRED_CMD.IBPB  = 0 (1)
00:00:02.837470   STIBP - Supports IA32_SPEC_CTRL.STIBP                   = 0 (1)
00:00:02.837472   FLUSH_CMD - Supports IA32_FLUSH_CMD                     = 1 (1)
00:00:02.837473   ARCHCAP - Supports IA32_ARCH_CAP                        = 0 (0)
00:00:02.837474   CORECAP - Supports IA32_CORE_CAP                        = 0 (0)
00:00:02.837476   SSBD - Supports IA32_SPEC_CTRL.SSBD                     = 0 (1)
00:00:02.837478  Sub-leaf 2
00:00:02.837478   Mnemonic - Description                                  = Guest (Host)
00:00:02.837479   PSFD - Supports IA32_SPEC_CTRL[7] (PSFD)                = 0 (0)
00:00:02.837481   IPRED_CTRL - Supports IA32_SPEC_CTRL[4:3] (IPRED_DIS)   = 0 (0)
00:00:02.837482   RRSBA_CTRL - Supports IA32_SPEC_CTRL[6:5] (RRSBA_DIS)   = 0 (0)
00:00:02.837483   DDPD_U - Supports IA32_SPEC_CTRL[8] (DDPD_U)            = 0 (0)
00:00:02.837484   BHI_CTRL - Supports IA32_SPEC_CTRL[10] (BHI_DIS_S)      = 0 (0)
00:00:02.837485   MCDT_NO - No MXCSR Config Dependent Timing issues       = 0 (0)
00:00:02.837486   UC_LOCK_DIS - Supports UC-lock disable and causing #AC  = 0 (0)
00:00:02.837487   MONITOR_MITG_NO - No MONITOR/UMONITOR power issues      = 0 (0)
00:00:02.837488 Processor Extended State Enumeration (leaf 0xd):
00:00:02.837489    XSAVE area cur/max size by XCR0, Guest: 0x340/0x340
00:00:02.837490    XSAVE area cur/max size by XCR0,  Host: 0x340/0x340
00:00:02.837492                    Valid XCR0 bits, Guest: 0x00000000`00000007 ( x87 SSE YMM_Hi128 )
