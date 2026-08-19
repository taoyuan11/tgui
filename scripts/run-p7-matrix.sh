#!/usr/bin/env sh
set -eu

report_path="${1:-target/p7-benchmarks/current.csv}"
baseline_path="${2:-}"
regression_percent="${TGUI_BENCH_REGRESSION_PERCENT:-20}"
report_dir=$(dirname "${report_path}")
mkdir -p "${report_dir}"

cargo bench --bench p7_matrix --no-default-features >"${report_path}"
stress_path="${report_path%.csv}-stress.csv"
cargo bench --bench p7_stress --all-features >"${stress_path}"
echo "wrote ${report_path}"
echo "wrote ${stress_path}"

if [ -z "${baseline_path}" ]; then
    exit 0
fi
if [ ! -f "${baseline_path}" ]; then
    echo "baseline does not exist: ${baseline_path}" >&2
    exit 2
fi

awk -F, -v threshold="${regression_percent}" '
    BEGIN { failed = 0 }
    FNR == NR && $1 !~ /^#/ && $1 != "nodes" {
        baseline[$1 "," $2] = $5
        next
    }
    $1 !~ /^#/ && $1 != "nodes" {
        key = $1 "," $2
        if (!(key in baseline)) {
            printf "missing baseline row: %s\n", key > "/dev/stderr"
            failed = 1
            next
        }
        limit = baseline[key] * (100 + threshold) / 100
        if ($5 > limit) {
            printf "p95 regression: %s baseline=%s current=%s threshold=%s%%\n", key, baseline[key], $5, threshold > "/dev/stderr"
            failed = 1
        }
    }
    END { exit failed }
' "${baseline_path}" "${report_path}"

echo "p95 total-time regression check passed (${regression_percent}% threshold)"
