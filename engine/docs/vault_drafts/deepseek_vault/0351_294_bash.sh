mkdir -p ~/fpga_all_files

find . -type f \( \
-name "*.zig" -o -name "*.md" -o -name "*.pdf" -o \
-name "*.v" -o -name "*.vh" -o -name "*.sv" -o -name "*.svh" -o \
-name "*.vhd" -o -name "*.vhdl" -o \
-name "*.xdc" -o -name "*.sdc" -o -name "*.pcf" -o -name "*.lpf" -o \
-name "*.tcl" -o -name "*.do" -o -name "*.prj" -o -name "*.qsf" -o -name "*.xpr" -o \
-name "*.c" -o -name "*.cpp" -o -name "*.h" -o -name "*.hpp" -o \
-name "*.txt" -o -name "*.docx" -o \
-name "*.cfg" -o -name "*.ini" -o -name "*.json" -o -name "*.xml" \
\) -exec cp --parents {} ~/fpga_all_files/ \;
