#!/bin/sh
# AsterDrive 容器资源监控脚本
# 用法: docker exec <container> sh /monitor.sh [采样间隔秒数] [输出CSV路径]
#
# 示例:
#   docker exec asterdrive-asterdrive-1 sh /scripts/monitor.sh 5
#   docker exec asterdrive-asterdrive-1 sh /scripts/monitor.sh 5 /tmp/metrics.csv

INTERVAL="${1:-5}"
CSV_OUT="${2:-}"
PID=1

# 检测 cgroup 版本
if [ -f /sys/fs/cgroup/memory/memory.stat ]; then
    CGROUP_V=1
    CGROUP_MEM_DIR="/sys/fs/cgroup/memory"
elif [ -f /sys/fs/cgroup/memory.stat ]; then
    CGROUP_V=2
    CGROUP_MEM_DIR="/sys/fs/cgroup"
else
    CGROUP_V=0
fi

# 写 CSV 头
if [ -n "$CSV_OUT" ]; then
    echo "timestamp,rss_mb,cache_mb,heap_mb,cpu_pct,disk_read_mb,disk_write_mb,net_rx_mb,net_tx_mb" > "$CSV_OUT"
fi

# 上一次 CPU 采样值（用于计算 CPU%）
PREV_PROC_TICKS=0
PREV_SYS_TICKS=0
FIRST_SAMPLE=1

get_cgroup_mem() {
    if [ "$CGROUP_V" -eq 1 ]; then
        RSS_BYTES=$(awk '/^rss / {print $2}' "$CGROUP_MEM_DIR/memory.stat")
        CACHE_BYTES=$(awk '/^cache / {print $2}' "$CGROUP_MEM_DIR/memory.stat")
    elif [ "$CGROUP_V" -eq 2 ]; then
        RSS_BYTES=$(awk '/^anon / {print $2}' "$CGROUP_MEM_DIR/memory.stat")
        CACHE_BYTES=$(awk '/^file / {print $2}' "$CGROUP_MEM_DIR/memory.stat")
    else
        # fallback: 从 /proc/PID/status 读
        RSS_BYTES=$(awk '/^VmRSS/ {print $2 * 1024}' /proc/$PID/status)
        CACHE_BYTES=0
    fi
    RSS_MB=$(echo "$RSS_BYTES" | awk '{printf "%.1f", $1/1048576}')
    CACHE_MB=$(echo "$CACHE_BYTES" | awk '{printf "%.1f", $1/1048576}')
}

get_heap() {
    # 从 /proc/PID/smaps_rollup 读堆内存（Private_Dirty ≈ 实际堆）
    if [ -f /proc/$PID/smaps_rollup ]; then
        HEAP_KB=$(awk '/^Private_Dirty/ {sum+=$2} END {print sum}' /proc/$PID/smaps_rollup)
        HEAP_MB=$(echo "$HEAP_KB" | awk '{printf "%.1f", $1/1024}')
    else
        HEAP_MB="n/a"
    fi
}

get_cpu() {
    # 进程 ticks: /proc/PID/stat field 14(utime)+15(stime)
    PROC_TICKS=$(awk '{print $14+$15}' /proc/$PID/stat)
    # 系统总 ticks: /proc/stat cpu 行所有字段之和
    SYS_TICKS=$(awk '/^cpu / {s=0; for(i=2;i<=NF;i++) s+=$i; print s}' /proc/stat)

    if [ "$FIRST_SAMPLE" -eq 1 ]; then
        CPU_PCT="0.0"
        FIRST_SAMPLE=0
    else
        DELTA_PROC=$((PROC_TICKS - PREV_PROC_TICKS))
        DELTA_SYS=$((SYS_TICKS - PREV_SYS_TICKS))
        if [ "$DELTA_SYS" -gt 0 ]; then
            CPU_PCT=$(echo "$DELTA_PROC $DELTA_SYS" | awk '{printf "%.1f", $1/$2*100}')
        else
            CPU_PCT="0.0"
        fi
    fi
    PREV_PROC_TICKS=$PROC_TICKS
    PREV_SYS_TICKS=$SYS_TICKS
}

get_disk_io() {
    if [ -f /proc/$PID/io ]; then
        READ_BYTES=$(awk '/^read_bytes/ {print $2}' /proc/$PID/io)
        WRITE_BYTES=$(awk '/^write_bytes/ {print $2}' /proc/$PID/io)
        DISK_READ_MB=$(echo "$READ_BYTES" | awk '{printf "%.1f", $1/1048576}')
        DISK_WRITE_MB=$(echo "$WRITE_BYTES" | awk '{printf "%.1f", $1/1048576}')
    else
        DISK_READ_MB="n/a"
        DISK_WRITE_MB="n/a"
    fi
}

get_net_io() {
    # 容器内 eth0 流量
    if [ -f /proc/net/dev ]; then
        NET_LINE=$(awk '/eth0/ {print $2, $10}' /proc/net/dev)
        if [ -n "$NET_LINE" ]; then
            NET_RX_MB=$(echo "$NET_LINE" | awk '{printf "%.1f", $1/1048576}')
            NET_TX_MB=$(echo "$NET_LINE" | awk '{printf "%.1f", $2/1048576}')
        else
            NET_RX_MB="0.0"
            NET_TX_MB="0.0"
        fi
    else
        NET_RX_MB="n/a"
        NET_TX_MB="n/a"
    fi
}

# 表头
printf "\033[1m%-20s %8s %8s %8s %7s %10s %10s %9s %9s\033[0m\n" \
    "TIME" "RSS" "CACHE" "HEAP" "CPU%" "DISK_R" "DISK_W" "NET_RX" "NET_TX"

while true; do
    NOW=$(date '+%Y-%m-%d %H:%M:%S')

    get_cgroup_mem
    get_heap
    get_cpu
    get_disk_io
    get_net_io

    printf "%-20s %7s M %7s M %7s M %6s%% %9s M %9s M %8s M %8s M\n" \
        "$NOW" "$RSS_MB" "$CACHE_MB" "$HEAP_MB" "$CPU_PCT" \
        "$DISK_READ_MB" "$DISK_WRITE_MB" "$NET_RX_MB" "$NET_TX_MB"

    if [ -n "$CSV_OUT" ]; then
        echo "$NOW,$RSS_MB,$CACHE_MB,$HEAP_MB,$CPU_PCT,$DISK_READ_MB,$DISK_WRITE_MB,$NET_RX_MB,$NET_TX_MB" >> "$CSV_OUT"
    fi

    sleep "$INTERVAL"
done
