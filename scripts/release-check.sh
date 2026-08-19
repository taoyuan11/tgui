#!/usr/bin/env sh
set -eu

mode="${1:---full}"
report_dir="${TGUI_RELEASE_REPORT_DIR:-target/p7-release-report}"
mkdir -p "${report_dir}"

case "${mode}" in
    --quick|--full) ;;
    *)
        echo "usage: scripts/release-check.sh [--quick|--full]" >&2
        exit 2
        ;;
esac

metadata_file="${report_dir}/metadata.txt"
{
    echo "schema=tgui-p7-release-v1"
    echo "commit=$(git rev-parse HEAD)"
    echo "dirty_files=$(git status --short | wc -l | tr -d ' ')"
    echo "rustc=$(rustc --version)"
    echo "cargo=$(cargo --version)"
    echo "host=$(rustc -vV | sed -n 's/^host: //p')"
    echo "profile=release,lto=thin,codegen-units=1,strip=symbols"
} >"${metadata_file}"

cargo fmt --all -- --check
cargo clippy --all-targets --no-default-features -- -D warnings
cargo clippy --all-targets --all-features -- -D warnings
scripts/check-features.sh
cargo test --no-default-features
cargo test
cargo test --all-features
cargo run --example p7_headless --no-default-features
cargo run --example p7_headless --all-features

if [ "${mode}" = "--quick" ]; then
    echo "quick release checks passed; metadata: ${metadata_file}"
    exit 0
fi

cargo doc --all-features --no-deps
cargo package --allow-dirty --no-verify

artifact_table="${report_dir}/artifact-sizes.csv"
echo "configuration,bytes,features" >"${artifact_table}"

build_artifact() {
    name="$1"
    features="$2"
    shift 2
    target_dir="${report_dir}/build-${name}"
    CARGO_TARGET_DIR="${target_dir}" cargo build --release --example p7_headless "$@"
    executable="${target_dir}/release/examples/p7_headless"
    if [ -f "${executable}.exe" ]; then
        executable="${executable}.exe"
    fi
    artifact="${report_dir}/p7_headless-${name}"
    cp "${executable}" "${artifact}"
    if size=$(stat -f %z "${artifact}" 2>/dev/null); then
        :
    else
        size=$(stat -c %s "${artifact}")
    fi
    echo "${name},${size},${features}" >>"${artifact_table}"
    CARGO_TARGET_DIR="${target_dir}" cargo tree "$@" >"${report_dir}/dependencies-${name}.txt"
}

build_artifact minimal none --no-default-features
build_artifact default default
build_artifact accessibility accessibility --no-default-features --features accessibility
build_artifact webview webview --no-default-features --features webview
build_artifact all all --all-features

echo "full release checks passed; report directory: ${report_dir}"
