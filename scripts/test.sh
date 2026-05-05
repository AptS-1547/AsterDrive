while true; do
  pid=$(pgrep aster_drive)
  rss_vsz=$(ps -o rss=,vsz= -p $pid 2>/dev/null)
  heap=$(curl -s http://localhost:3000/health/memory 2>/dev/null | grep -o '"heap_allocated_mb":"[^"]*"' | cut -d'"' -f4)
  peak=$(curl -s http://localhost:3000/health/memory 2>/dev/null | grep -o '"heap_peak_mb":"[^"]*"' | cut -d'"' -f4)
  dirty=$(vmmap $pid 2>/dev/null | grep "TOTAL " | head -1 | awk '{print $4}')
  echo "$rss_vsz" | awk -v h="$heap" -v p="$peak" -v d="$dirty" '{printf "RSS: %.1f MB | Dirty: %s | Heap: %s MB (peak: %s MB)\n", $1/1024, d, h, p}'
  sleep 1
done