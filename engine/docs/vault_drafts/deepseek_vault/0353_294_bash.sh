# Найти все файлы, в которых встречается "module" (для Verilog) или "entity" (VHDL)
grep -rl --include="" "module" . | while read -r file; do
cp --parents "$file" ~/fpga_all_files/
done
