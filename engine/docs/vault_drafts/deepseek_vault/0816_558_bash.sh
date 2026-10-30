for f in *; do
if [ -f "$f" ]; then
echo "===== $f ====="
cat "$f"
echo
fi
