#!/usr/bin/env bash
# 扫描性能基准：耗时 / 峰值 RSS / 与 du -sB1 的误差。用法：scripts/bench.sh [路径] [次数]
# 不依赖 GNU time：RSS 由 example 自己读 /proc/self/status 的 VmHWM。
set -euo pipefail

root="${1:-$HOME/Projects}"
iters="${2:-3}"

cd "$(dirname "$0")/.."
cargo build --release --example scan -p disklens-core >/dev/null
bin="target/release/examples/scan"

du_bytes=$(du -sB1 "$root" 2>/dev/null | cut -f1 || true)
du_bytes=${du_bytes:-0}
echo "path      $root"
echo "du -sB1   $du_bytes"
echo "runs      $iters"

for i in $(seq 1 "$iters"); do
    out=$(mktemp)
    start=$(date +%s%N)
    "$bin" "$root" >"$out"
    end=$(date +%s%N)
    wall_ms=$(( (end - start) / 1000000 ))
    scan_ms=$(awk '/^elapsed ms/{print $3}' "$out")
    rss=$(awk '/^peak rss kb/{print $4}' "$out")
    total=$(awk '/^total bytes/{print $3}' "$out")
    entries=$(awk '/^entries/{print $2}' "$out")
    pct=$(awk -v a="$total" -v b="$du_bytes" 'BEGIN { if (b==0) print "n/a"; else printf "%+.3f%%", (a-b)/b*100 }')
    app=$(awk '/^du apparent/{print $3}' "$out")
    app_pct=$(awk -v a="$total" -v b="$app" 'BEGIN { if (b=="") print "n/a"; else printf "%+.3f%%", (a-b)/b*100 }')
    echo "run $i: wall=${wall_ms}ms scan=${scan_ms}ms peak_rss=${rss}KB entries=$entries bytes=$total vs_du=$pct vs_du_apparent=$app_pct"
    rm -f "$out"
done
