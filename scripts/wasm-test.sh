#!/usr/bin/env bash

set -Eeuo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly WASM_TARGET="wasm32-unknown-unknown"

# kind|package|target|features|source
# features 为空表示默认 feature；非空值会作为 --features 的参数传递。
readonly WASM_TESTS=(
    "test|silex|accordion||crates/silex/tests/accordion.rs"
    "test|silex|error_boundary||crates/silex/tests/error_boundary.rs"
    "test|silex|portal||crates/silex/tests/portal.rs"
    "test|silex|tw_tests||crates/silex/tests/tw_tests.rs"
    "lib|silex_bootstrap||js-object|crates/silex_bootstrap/src/js_object.rs"
    "test|silex_bootstrap|app_host||crates/silex_bootstrap/tests/app_host.rs"
    "test|silex_bootstrap|browser_bootstrap|browser-bootstrap|crates/silex_bootstrap/tests/browser_bootstrap.rs"
    "test|silex_bootstrap|docs_examples||crates/silex_bootstrap/tests/docs_examples.rs"
    "test|silex_bootstrap|js_object|js-object|crates/silex_bootstrap/tests/js_object.rs"
    "test|silex_bootstrap|page_controller|page-controller|crates/silex_bootstrap/tests/page_controller.rs"
    "test|silex_core|async_completion||crates/silex_core/tests/async_completion.rs"
    "test|silex_css|fallback|test-style-fallback|crates/silex_css/tests/fallback.rs"
    "test|silex_css|owner||crates/silex_css/tests/owner.rs"
    "test|silex_dom|docs_examples||crates/silex_dom/tests/docs_examples.rs"
    "test|silex_dom|host_resources||crates/silex_dom/tests/host_resources.rs"
    "test|silex_dom|mounted_app||crates/silex_dom/tests/mounted_app.rs"
    "test|silex_dom|owner||crates/silex_dom/tests/owner.rs"
    "test|silex_dom|reactive_attribute||crates/silex_dom/tests/reactive_attribute.rs"
    "test|silex_html|browser||crates/silex_html/tests/browser.rs"
    "test|silex_i18n|browser|browser-tests,intl|crates/silex_i18n/tests/browser.rs"
    "test|silex_i18n|wasm||crates/silex_i18n/tests/wasm.rs"
    "test|silex_net|browser|json,persist|crates/silex_net/tests/browser.rs"
    "test|silex_persist|browser||crates/silex_persist/tests/browser.rs"
    "test|silex_router|router||crates/silex_router/tests/router.rs"
    "test|silex_macros_test|macro_owner||crates/tests/silex_macros_test/tests/macro_owner.rs"
    "test|silex_counter|browser||examples/counter/tests/browser.rs"
    "test|silex_error_demo|browser||examples/error_boundary_demo/tests/browser.rs"
    "test|silex_router_example|browser||examples/router/tests/browser.rs"
    "test|silex_showcase|browser||examples/showcase/tests/browser.rs"
    "test|silex_store_demo|browser||examples/store_demo/tests/browser.rs"
    "test|silex_ui_example|browser||examples/ui/tests/browser.rs"
)

DRIVER_PID=""
DRIVER_LOG=""

usage() {
    cat <<'EOF'
用法：
  scripts/wasm-test.sh --list
  scripts/wasm-test.sh

默认行为：
  使用 nightly、build-std 和 Firefox headless，顺序执行所有 Wasm 测试。

可选环境变量：
  WASM_TEST_FAST_FAIL       是否快速失败，默认 1；设置为 0 可执行完剩余测试
  GECKODRIVER_REMOTE       使用已经运行的 WebDriver，不启动新的 geckodriver
  GECKODRIVER               指定 geckodriver 的绝对路径
  WASM_TEST_GECKODRIVER_PORT  本地 geckodriver 端口，默认 4444
  WASM_TEST_GECKODRIVER_LOG   geckodriver 日志级别，默认 info
EOF
}

die() {
    printf '错误：%s\n' "$1" >&2
    exit 1
}

cleanup() {
    local exit_status=$?

    if [[ -n "${DRIVER_PID}" ]]; then
        if kill -0 "${DRIVER_PID}" 2>/dev/null; then
            kill "${DRIVER_PID}" 2>/dev/null || true
        fi
        wait "${DRIVER_PID}" 2>/dev/null || true
    fi

    if [[ -n "${DRIVER_LOG}" ]]; then
        rm -f -- "${DRIVER_LOG}"
    fi

    exit "${exit_status}"
}

print_tests() {
    printf '共 %d 个 Wasm 测试入口：\n\n' "${#WASM_TESTS[@]}"
    printf '%-3s %-10s %-24s %-24s %s\n' '#' '包' '测试 target' '测试类型' '源码'

    local index=1
    local record
    local kind
    local package_name
    local target_name
    local feature_args
    local source_path
    local feature_display

    for record in "${WASM_TESTS[@]}"; do
        IFS='|' read -r kind package_name target_name feature_args source_path <<< "${record}"
        feature_display="${feature_args:-default}"
        printf '%-3d %-10s %-24s %-24s %s\n' \
            "${index}" \
            "${package_name}" \
            "${target_name:-库测试}" \
            "${kind}（${feature_display}）" \
            "${source_path}"
        ((index += 1))
    done
}

require_command() {
    local command_name="$1"

    command -v "${command_name}" >/dev/null 2>&1 \
        || die "找不到命令：${command_name}"
}

wait_for_webdriver() {
    local remote_url="$1"
    local status_url="${remote_url%/}/status"
    local attempt

    for ((attempt = 1; attempt <= 50; attempt += 1)); do
        if curl --fail --silent --show-error --max-time 1 "${status_url}" >/dev/null; then
            return
        fi

        if [[ -n "${DRIVER_PID}" ]] \
            && ! kill -0 "${DRIVER_PID}" 2>/dev/null; then
            printf 'geckodriver 启动失败，日志如下：\n' >&2
            cat -- "${DRIVER_LOG}" >&2
            exit 1
        fi

        sleep 0.2
    done

    printf '等待 WebDriver 启动超时，日志如下：\n' >&2
    if [[ -n "${DRIVER_LOG}" ]]; then
        cat -- "${DRIVER_LOG}" >&2
    fi
    exit 1
}

start_webdriver() {
    local remote_url

    if [[ -n "${GECKODRIVER_REMOTE:-}" ]]; then
        remote_url="${GECKODRIVER_REMOTE%/}"
        printf '使用已经运行的 WebDriver：%s\n' "${remote_url}"
    else
        local driver_binary="${GECKODRIVER:-}"
        local driver_port="${WASM_TEST_GECKODRIVER_PORT:-4444}"
        local driver_log_level="${WASM_TEST_GECKODRIVER_LOG:-info}"

        if [[ -z "${driver_binary}" ]]; then
            driver_binary="$(command -v geckodriver || true)"
        fi
        [[ -n "${driver_binary}" ]] \
            || die '找不到 geckodriver，请设置 GECKODRIVER 或 GECKODRIVER_REMOTE'

        DRIVER_LOG="$(mktemp -t silex-wasm-geckodriver.XXXXXX.log)"
        printf '启动 geckodriver：%s\n' "${driver_binary}"
        "${driver_binary}" \
            --port "${driver_port}" \
            --log "${driver_log_level}" \
            >"${DRIVER_LOG}" 2>&1 &
        DRIVER_PID=$!
        remote_url="http://127.0.0.1:${driver_port}"
    fi

    wait_for_webdriver "${remote_url}"
    export GECKODRIVER_REMOTE="${remote_url}"
}

run_test() {
    local record="$1"
    local kind
    local package_name
    local target_name
    local feature_args
    local source_path
    local test_label
    local -a cargo_command

    IFS='|' read -r kind package_name target_name feature_args source_path <<< "${record}"
    test_label="${package_name}/${target_name:-lib}"

    cargo_command=(cargo +nightly wasm-test-nightly -p "${package_name}")
    if [[ -n "${feature_args}" ]]; then
        cargo_command+=(--features "${feature_args}")
    fi

    if [[ "${kind}" == 'lib' ]]; then
        cargo_command+=(--lib)
    else
        cargo_command+=(--test "${target_name}")
    fi
    cargo_command+=(-- --include-ignored --nocapture)

    printf '\n[%s] %s\n' "${test_label}" "${source_path}"
    "${cargo_command[@]}"
}

main() {
    cd -- "${REPO_ROOT}"

    case "${1:-}" in
        --list)
            [[ "$#" -eq 1 ]] || die '--list 不能和其他参数一起使用'
            print_tests
            return
            ;;
        '')
            ;;
        --help|-h)
            usage
            return
            ;;
        *)
            usage >&2
            exit 2
            ;;
    esac

    require_command cargo
    require_command curl
    require_command firefox
    require_command rustup
    require_command wasm-bindgen-test-runner

    cargo +nightly --version >/dev/null \
        || die 'nightly 工具链不可用，请先安装 nightly'
    local nightly_components
    local installed_targets
    nightly_components="$(rustup component list --toolchain nightly --installed)"
    grep -q '^rust-src' <<< "${nightly_components}" \
        || die 'nightly 缺少 rust-src，请执行：rustup component add rust-src --toolchain nightly'
    installed_targets="$(rustup target list --installed)"
    grep -qx "${WASM_TARGET}" <<< "${installed_targets}" \
        || die "缺少 ${WASM_TARGET} target，请执行：rustup target add ${WASM_TARGET}"

    if [[ -z "${GECKODRIVER_REMOTE:-}" ]]; then
        if [[ -z "${GECKODRIVER:-}" ]]; then
            require_command geckodriver
        else
            [[ -x "${GECKODRIVER}" ]] \
                || die "GECKODRIVER 不是可执行文件：${GECKODRIVER}"
        fi
    fi

    export WASM_BINDGEN_USE_BROWSER=1
    export NO_PROXY="${NO_PROXY:+${NO_PROXY},}127.0.0.1,localhost"
    export no_proxy="${no_proxy:+${no_proxy},}127.0.0.1,localhost"
    export RUSTFLAGS="${RUSTFLAGS:+${RUSTFLAGS} }-D warnings -Cpanic=unwind -Cllvm-args=-wasm-use-legacy-eh=false"

    local fast_fail="${WASM_TEST_FAST_FAIL:-1}"
    case "${fast_fail}" in
        1|true|yes|on)
            fast_fail=1
            ;;
        0|false|no|off)
            fast_fail=0
            ;;
        *)
            die 'WASM_TEST_FAST_FAIL 只能设置为 1/0、true/false、yes/no 或 on/off'
            ;;
    esac

    start_webdriver

    printf '开始执行 %d 个 Wasm 测试，工具链：nightly\n' "${#WASM_TESTS[@]}"
    if ((fast_fail)); then
        printf 'fast fail：启用\n'
    else
        printf 'fast fail：禁用，将执行剩余测试并在末尾汇总失败项\n'
    fi

    local failed_count=0
    local record
    local index=1
    local -a failed_tests=()

    for record in "${WASM_TESTS[@]}"; do
        printf '\n进度：%d/%d\n' "${index}" "${#WASM_TESTS[@]}"
        if ! run_test "${record}"; then
            IFS='|' read -r _ package_name target_name _ _ <<< "${record}"
            failed_tests+=("${package_name}/${target_name:-lib}")
            ((failed_count += 1))
            if ((fast_fail)); then
                printf '\nfast fail：%s 失败，停止后续测试。\n' \
                    "${package_name}/${target_name:-lib}" >&2
                exit 1
            fi
        fi
        ((index += 1))
    done

    if ((failed_count > 0)); then
        printf '\nWasm 测试完成，但有 %d 个入口失败：\n' "${failed_count}" >&2
        printf '  - %s\n' "${failed_tests[@]}" >&2
        exit 1
    fi

    printf '\n全部 %d 个 Wasm 测试入口执行成功。\n' "${#WASM_TESTS[@]}"
}

trap cleanup EXIT
main "$@"
