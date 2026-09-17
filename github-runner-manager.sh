#!/bin/bash
# github-runner-manager.sh
# Usage:
#   Deploy all runners:  sudo bash github-runner-manager.sh deploy  <GITHUB_TOKEN>
#   Destroy all runners: sudo bash github-runner-manager.sh destroy <GITHUB_TOKEN>
#
# The token is a short-lived GitHub Actions registration/removal token,
# obtained from the repo's Settings > Actions > Runners page (or via the API).
# Upgrade manually: download the new tarball, destroy, change RUNNER_TARBALL,
# then deploy.

set -euo pipefail

# This script installs systemd services and edits /etc/systemd, so it must run
# as root. config.sh is later invoked as SVC_USER (it refuses to run as root).
if [[ $EUID -ne 0 ]]; then
    echo "This script must be run as root (use sudo)." >&2
    exit 1
fi

# ---- Configuration ---------------------------------------------------------
RUNNER_PREFIX="runner"
RUNNER_BASE_DIR="/home/runner/runners"
SVC_USER="runner"
RUNNER_TARBALL="actions-runner-linux-x64-2.337.0.tar.gz"
GITHUB_URL="https://github.com/asterinas/asterinas"
RUNNER_LABEL="tdx"
RUNNER_NAME="${RUNNER_LABEL}-runner"
MAX_RUNNERS=5                     # Number of runners to deploy
PHYSICAL_CORES_PER_RUNNER=8       # Physical cores dedicated to each runner
                                  # 5 runners x 8 = 40 physical cores

# HTTP(S) proxy the runner uses to reach GitHub (github.com, codeload.github.com,
# api.github.com, ...).
HOST_HTTPS_PROXY="http://127.0.0.1:7890"
RUNNER_HTTPS_PROXY="http://172.17.0.1:7890"
RUNNER_NO_PROXY="localhost,127.0.0.1,*.local,192.168.*.*"
HEALTH_TIMEOUT_SECONDS=60
HEALTH_STABILITY_SECONDS=10
STATE_DIR="${RUNNER_BASE_DIR}/.runner-manager"
LOCK_FILE="/run/lock/github-runner-manager.lock"
SYSTEMD_DIR="/etc/systemd/system"

# Action-archive cache. The runner checks this dir before downloading an action
# from codeload.github.com: a hit is copied from local disk (zero network).
# Layout it looks up (from the runner source): <dir>/<owner>_<repo>/<sha>.tar.gz
ACTION_CACHE_DIR="${RUNNER_BASE_DIR}/action-cache"
# Actions to pre-populate. Pin to a SHA (owner/repo@<sha>) for a deterministic
# cache hit; a tag/branch is re-resolved to a SHA each refresh and may miss later.
# Include all actions referenced by the benchmark composite, including its
# conditional steps: preparation can download them before evaluating conditions.
# This cache only covers action source archives, not checkout, images, or artifacts.
ACTION_CACHE_LIST=(
    "actions/checkout@v4"
    "actions/upload-artifact@v4"
    "actions/download-artifact@v4"
    "asterinas/github-action-benchmark@v5"
)

# ---- CPU topology (auto-detected) -----------------------------------------
# Build an ordered list of physical cores, each entry being that core's full
# set of logical CPUs (its hyperthread siblings), e.g. "0,32". Reading the
# real sibling list avoids assuming any particular CPU numbering scheme.
declare -a PHYS_CORES=()
detect_cpu_topology() {
    local cpu siblings_file siblings
    local -A seen=()
    PHYS_CORES=()
    while IFS= read -r cpu; do
        siblings_file="${cpu}/topology/thread_siblings_list"
        [[ -r "${siblings_file}" ]] || continue
        siblings=$(<"${siblings_file}")
        if [[ -z "${seen[${siblings}]:-}" ]]; then
            seen[${siblings}]=1
            PHYS_CORES+=("${siblings}")
        fi
    done < <(printf '%s\n' /sys/devices/system/cpu/cpu[0-9]* | sort -V)
    if (( ${#PHYS_CORES[@]} == 0 )); then
        echo "Error: failed to detect CPU topology" >&2
        return 1
    fi
}

# Compute the logical-CPU range for runner ${num} (1-based). Prints the
# taskset core spec (e.g. "0,32,1,33") on success, or fails if there aren't
# enough physical cores for the requested runner.
cpu_range_for_runner() {
    local num=$1
    local start=$(( (num - 1) * PHYSICAL_CORES_PER_RUNNER ))
    local end=$(( start + PHYSICAL_CORES_PER_RUNNER - 1 ))

    if (( end >= ${#PHYS_CORES[@]} )); then
        echo "Error: runner ${num} needs physical cores ${start}-${end}," \
             "but only ${#PHYS_CORES[@]} are available" >&2
        return 1
    fi

    local spec="" core_index
    for (( core_index = start; core_index <= end; core_index++ )); do
        spec+="${PHYS_CORES[core_index]},"
    done
    echo "${spec%,}"
}

atomic_write() (
    local destination=$1 temporary
    temporary=$(mktemp "${destination}.tmp.XXXXXX") || return 1
    trap 'rm -f -- "${temporary}"' EXIT
    cat > "${temporary}" || return 1
    chmod 644 "${temporary}" || return 1
    mv -f -- "${temporary}" "${destination}"
)

record_runner() {
    local runner_root=$1
    local instance="${STATE_DIR}/instances/$(basename "${runner_root}")"
    mkdir -p "${instance}" || return 1
    printf '%s\n' "${runner_root}/actions-runner" | atomic_write "${instance}/runner-dir" || return 1
    if [[ -f "${runner_root}/actions-runner/.service" ]]; then
        atomic_write "${instance}/service" < "${runner_root}/actions-runner/.service" || return 1
    fi
}

discover_runners() {
    local runner_root suffix
    mkdir -p "${STATE_DIR}/instances"
    for runner_root in "${RUNNER_BASE_DIR}/${RUNNER_PREFIX}"*; do
        [[ -d "${runner_root}/actions-runner" ]] || continue
        suffix=${runner_root##*/${RUNNER_PREFIX}}
        [[ "${suffix}" =~ ^[0-9]+$ ]] || continue
        record_runner "${runner_root}"
    done
}

runner_service_name() {
    local instance=$1 runner_dir service_name
    runner_dir=$(<"${instance}/runner-dir")
    if [[ -f "${runner_dir}/.service" ]]; then
        service_name=$(<"${runner_dir}/.service")
    elif [[ -f "${instance}/service" ]]; then
        service_name=$(<"${instance}/service")
    elif [[ -f "${runner_dir}/svc.sh" ]]; then
        service_name=$(sed -n 's/^SVC_NAME="\([^"]*\)"$/\1/p' "${runner_dir}/svc.sh")
    else
        return 1
    fi
    if [[ ! "${service_name}" =~ ^actions\.runner\.[a-zA-Z0-9_.-]+\.service$ ]]; then
        echo "Error: invalid service name for ${runner_dir}" >&2
        return 1
    fi
    printf '%s\n' "${service_name}"
}

configure_docker_proxy() {
    [[ -n "${HOST_HTTPS_PROXY}" ]] || return 0
    local proxy_dir="${SYSTEMD_DIR}/docker.service.d"
    local daemon_proxy
    mkdir -p "${proxy_dir}"
    atomic_write "${proxy_dir}/00-runner-manager-proxy.conf" <<EOF
[Service]
Environment="HTTP_PROXY=${HOST_HTTPS_PROXY}"
Environment="HTTPS_PROXY=${HOST_HTTPS_PROXY}"
Environment="NO_PROXY=${RUNNER_NO_PROXY}"
EOF
    systemctl daemon-reload
    if systemctl is-active --quiet docker.service; then
        daemon_proxy=$(docker info --format '{{.HTTPProxy}}|{{.HTTPSProxy}}')
        if [[ ! "${daemon_proxy}" =~ ^[^\|]+\|[^\|]+$ ]]; then
            echo "Docker proxy fallback installed. Restart docker.service during maintenance, then rerun deploy." >&2
            return 1
        fi
    fi
}

write_docker_wrapper() (
    local instance=$1 core_range=$2 docker_binary=$3 temporary
    mkdir -p "${instance}/bin"
    temporary=$(mktemp "${instance}/bin/docker.tmp.XXXXXX")
    trap 'rm -f -- "${temporary}"' EXIT
    {
        printf '#!/bin/bash\nset -euo pipefail\n'
        printf 'docker_binary=%q\ncore_range=%q\n' "${docker_binary}" "${core_range}"
        cat <<'EOF'
case "${1:-}" in
    create|run)
        command_name=$1
        shift
        exec "${docker_binary}" "${command_name}" --cpuset-cpus "${core_range}" "$@"
        ;;
    *)
        exec "${docker_binary}" "$@"
        ;;
esac
EOF
    } > "${temporary}"
    chmod 755 "${temporary}"
    mv -f -- "${temporary}" "${instance}/bin/docker"
)

runner_started() {
    local service_name=$1 invocation_id=${2:-} logs current_invocation
    if [[ -z "${invocation_id}" ]]; then
        invocation_id=$(systemctl show "${service_name}" --property=InvocationID --value) || return 1
    fi
    [[ -n "${invocation_id}" ]] || return 1
    logs=$(journalctl --unit="${service_name}" "_SYSTEMD_INVOCATION_ID=${invocation_id}" \
        --no-pager --output=cat) || return 1
    if [[ "${logs}" == *"is deprecated and cannot receive messages"* ]]; then
        echo "Error: GitHub rejected the installed runner version. Download a supported runner, destroy the old instances, and deploy the new tarball." >&2
        return 2
    fi
    if [[ "${logs}" == *"Runner listener exit with terminated error"* ]]; then
        echo "Error: ${service_name} reported a terminal listener error." >&2
        return 2
    fi
    systemctl is-active --quiet "${service_name}" || return 1
    current_invocation=$(systemctl show "${service_name}" --property=InvocationID --value) || return 1
    [[ "${current_invocation}" == "${invocation_id}" ]] || return 1
    [[ "${logs}" == *"Listening for Jobs"* ]]
}

wait_for_runner() {
    local service_name=$1 deadline=$((SECONDS + HEALTH_TIMEOUT_SECONDS))
    local stable_since=-1 stable_invocation="" invocation_id health_status service_state
    local failure_reason="did not stay ready within ${HEALTH_TIMEOUT_SECONDS}s"
    while (( SECONDS < deadline )); do
        invocation_id=$(systemctl show "${service_name}" --property=InvocationID --value) || return 1
        if runner_started "${service_name}" "${invocation_id}"; then
            if (( stable_since < 0 )) || [[ "${stable_invocation}" != "${invocation_id}" ]]; then
                stable_since=$SECONDS
                stable_invocation=${invocation_id}
            fi
            if (( SECONDS - stable_since >= HEALTH_STABILITY_SECONDS )); then
                echo "Ready: ${service_name} passed its ${HEALTH_STABILITY_SECONDS}s startup check."
                return 0
            fi
        else
            health_status=$?
            stable_since=-1
            if (( health_status == 2 )); then
                failure_reason="reported a terminal runner error"
                break
            fi
        fi
        service_state=$(systemctl show "${service_name}" --property=ActiveState --value) || return 1
        if [[ "${service_state}" == inactive || "${service_state}" == failed ]]; then
            failure_reason="exited before passing its startup check"
            break
        fi
        sleep 2
    done
    echo "Error: ${service_name} ${failure_reason}; keeping state for retry." >&2
    journalctl --unit="${service_name}" --lines=20 --no-pager >&2 || true
    return 1
}

# ---- Deploy ----------------------------------------------------------------
deploy_runner() {
    local num=$1
    local token=$2
    local runner_dir="${RUNNER_BASE_DIR}/${RUNNER_PREFIX}${num}/actions-runner"
    local runner_name="${RUNNER_NAME}-${num}"
    local instance="${STATE_DIR}/instances/${RUNNER_PREFIX}${num}"

    echo "Deploying ${runner_name}..."

    # Fail fast, before touching GitHub or systemd, if this runner can't be
    # pinned to a valid set of cores.
    local core_range
    core_range=$(cpu_range_for_runner "${num}")

    record_runner "${RUNNER_BASE_DIR}/${RUNNER_PREFIX}${num}"
    mkdir -p "${runner_dir}"

    # Extract base files (only on first run).
    if [[ ! -f "${runner_dir}/run.sh" ]]; then
        local tarball="${RUNNER_BASE_DIR}/${RUNNER_TARBALL}"
        if [[ ! -f "${tarball}" ]]; then
            echo "Error: runner tarball not found at ${tarball}" >&2
            return 1
        fi
        tar xzf "${tarball}" -C "${runner_dir}"
    fi

    if [[ -f "${runner_dir}/.runner" ]]; then
        python3 - "${runner_dir}/.runner" "${GITHUB_URL}" "${runner_name}" "_work-${num}" <<'PY'
import json
import sys

with open(sys.argv[1]) as config_file:
    settings = json.load(config_file)
expected = dict(zip(("gitHubUrl", "agentName", "workFolder"), sys.argv[2:]))
if any(settings.get(key) != value for key, value in expected.items()):
    sys.exit("Existing runner registration differs from configuration; destroy it before reconfiguring.")
PY
        if [[ ! -f "${runner_dir}/.credentials" ]]; then
            echo "Error: registered runner has no .credentials: ${runner_dir}" >&2
            return 1
        fi
    fi

    local service_name=""
    if [[ -f "${runner_dir}/svc.sh" ]]; then
        service_name=$(runner_service_name "${instance}")
        printf '%s\n' "${service_name}" | atomic_write "${instance}/service"
        if [[ -f "${SYSTEMD_DIR}/${service_name}" ]]; then
            systemctl stop "${service_name}"
        fi
    fi

    # The runner and its containers can both reach this proxy through docker0.
    {
        if [[ -n "${RUNNER_HTTPS_PROXY}" ]]; then
            echo "https_proxy=${RUNNER_HTTPS_PROXY}"
            echo "http_proxy=${RUNNER_HTTPS_PROXY}"
            echo "no_proxy=${RUNNER_NO_PROXY}"
        fi
        if [[ -n "${ACTION_CACHE_DIR}" ]]; then
            echo "ACTIONS_RUNNER_ACTION_ARCHIVE_CACHE=${ACTION_CACHE_DIR}"
        fi
    } | atomic_write "${runner_dir}/.env"

    # config.sh and the runner service must run as an unprivileged user, so
    # hand ownership of the whole runner tree to SVC_USER before configuring.
    chown -R "${SVC_USER}:${SVC_USER}" "${RUNNER_BASE_DIR}/${RUNNER_PREFIX}${num}"

    # Configure runner as SVC_USER (config.sh refuses to run as root).
    if [[ ! -f "${runner_dir}/.runner" ]]; then
        (
            cd "${runner_dir}"
            sudo -u "${SVC_USER}" ./config.sh --url "${GITHUB_URL}" \
                        --token "${token}" \
                        --name "${runner_name}" \
                        --labels "${RUNNER_LABEL}" \
                        --work "_work-${num}" \
                        --replace \
                        --disableupdate \
                        --unattended
        )
    fi

    # svc.sh records the exact systemd unit name (e.g.
    # actions.runner.<owner>-<repo>.<runner_name>.service) in .service. Before
    # installation, recover it from the generated svc.sh or saved manager state.
    service_name=$(runner_service_name "${instance}")
    printf '%s\n' "${service_name}" | atomic_write "${instance}/service"
    local service_file="${SYSTEMD_DIR}/${service_name}"
    if [[ -f "${service_file}" ]] &&
       [[ ! -f "${runner_dir}/.service" || ! -f "${runner_dir}/runsvc.sh" ]]; then
        (cd "${runner_dir}"; ./svc.sh uninstall)
    fi
    if [[ ! -f "${service_file}" ]]; then
        (cd "${runner_dir}"; ./svc.sh install "${SVC_USER}")
    else
        printf '%s\n' "${service_name}" | atomic_write "${runner_dir}/.service"
        chown "${SVC_USER}:${SVC_USER}" "${runner_dir}/.service"
    fi

    local docker_binary runner_path
    docker_binary=$(command -v docker)
    write_docker_wrapper "${instance}" "${core_range}" "${docker_binary}"
    runner_path=$(<"${runner_dir}/.path")
    runner_path=${runner_path#"${instance}/bin:"}
    printf '%s\n' "${instance}/bin:${runner_path}" | atomic_write "${runner_dir}/.path"
    chown "${SVC_USER}:${SVC_USER}" "${runner_dir}/.path"

    mkdir -p "${service_file}.d"
    atomic_write "${service_file}.d/runner-manager.conf" <<EOF
[Unit]
Requires=docker.service
After=docker.service network-online.target

[Service]
ExecStart=
ExecStart="${runner_dir}/runsvc.sh"
CPUAffinity=
CPUAffinity=${core_range//,/ }
Restart=on-failure
RestartSec=5s
EOF

    systemctl daemon-reload
    systemctl enable "${service_name}"
    systemctl restart "${service_name}"
    wait_for_runner "${service_name}"
}

# ---- Destroy ---------------------------------------------------------------
# Best-effort cleanup: keep going even if an individual step fails, so one
# broken runner doesn't block tearing down the rest.
destroy_runner() {
    local instance=$1
    local token=$2
    local runner_dir service_name=""
    runner_dir=$(<"${instance}/runner-dir") || return 1
    if [[ "${runner_dir}" != "${RUNNER_BASE_DIR}/$(basename "${instance}")/actions-runner" ]]; then
        echo "Error: invalid runner directory recorded in ${instance}" >&2
        return 1
    fi

    echo "Destroying ${runner_dir}..."

    if [[ -f "${instance}/service" || -f "${runner_dir}/.service" || -f "${runner_dir}/svc.sh" ]]; then
        service_name=$(runner_service_name "${instance}") || return 1
        printf '%s\n' "${service_name}" | atomic_write "${instance}/service" || return 1
        if [[ -f "${SYSTEMD_DIR}/${service_name}" || -f "${runner_dir}/.service" ]]; then
            systemctl stop "${service_name}" || return 1
            (cd "${runner_dir}" && ./svc.sh uninstall) || return 1
        fi
    fi

    if [[ -f "${runner_dir}/.runner" || -f "${runner_dir}/.credentials" ]]; then
        (cd "${runner_dir}" && sudo -u "${SVC_USER}" ./config.sh remove --token "${token}") || return 1
    fi

    if [[ -n "${service_name}" ]]; then
        rm -f -- "${SYSTEMD_DIR}/${service_name}.d/runner-manager.conf" || return 1
        systemctl daemon-reload || return 1
    fi
    rm -rf -- "${runner_dir%/actions-runner}" || return 1
    rm -rf -- "${instance}"
}

show_status() {
    local instance service_name failed=0
    for instance in "${STATE_DIR}/instances/"*; do
        [[ -f "${instance}/runner-dir" ]] || continue
        if service_name=$(runner_service_name "${instance}") && runner_started "${service_name}"; then
            echo "${service_name}: active; GitHub connection confirmed at service startup."
        else
            echo "$(basename "${instance}"): inactive or startup connection not confirmed." >&2
            failed=1
        fi
        if [[ -n "${service_name}" ]]; then
            journalctl --unit="${service_name}" --lines=5 --no-pager || failed=1
        fi
    done
    return "${failed}"
}

# ---- Action archive cache --------------------------------------------------
# Pre-download each action's source tarball into ACTION_CACHE_DIR using the
# host proxy to avoid action source downloads on cache hits. Safe to re-run;
# valid existing archives are skipped. Optional GH_TOKEN raises
# the api.github.com rate limit when resolving tags to SHAs.
refresh_action_cache() (
    [[ -n "${ACTION_CACHE_DIR}" ]] || { echo "ACTION_CACHE_DIR unset" >&2; return 1; }

    local proxy_opt=()
    [[ -n "${HOST_HTTPS_PROXY}" ]] && proxy_opt=(--proxy "${HOST_HTTPS_PROXY}")
    local auth=()
    [[ -n "${GH_TOKEN:-}" ]] && auth=(-H "Authorization: Bearer ${GH_TOKEN}")

    local action_ref temporary=""
    trap '[[ -z "${temporary}" ]] || rm -f -- "${temporary}"' EXIT
    mkdir -p "${ACTION_CACHE_DIR}"
    chown -R root:root "${ACTION_CACHE_DIR}"
    find "${ACTION_CACHE_DIR}" -type d -exec chmod 755 {} +
    find "${ACTION_CACHE_DIR}" -type f -exec chmod 644 {} +
    for action_ref in "${ACTION_CACHE_LIST[@]}"; do
        local repo="${action_ref%@*}" ref="${action_ref##*@}"
        # Resolve tag/branch to the commit SHA the runner keys the cache on.
        local sha
        sha=$(curl -fsSL --retry 3 --connect-timeout 15 --max-time 60 "${proxy_opt[@]}" "${auth[@]}" \
            -H "Accept: application/vnd.github.sha" \
            "https://api.github.com/repos/${repo}/commits/${ref}") \
            || { echo "Failed to resolve ${action_ref}" >&2; return 1; }
        [[ "${sha}" =~ ^[0-9a-f]{40}$ ]] || { echo "Invalid commit SHA for ${action_ref}" >&2; return 1; }

        local dir="${ACTION_CACHE_DIR}/${repo/\//_}"
        local out="${dir}/${sha}.tar.gz"
        mkdir -p "${dir}"
        if [[ -f "${out}" ]] && tar tzf "${out}" >/dev/null 2>&1; then
            echo "cached  ${repo}@${sha}"
            continue
        fi
        echo "fetch   ${repo}@${ref} -> ${sha}"
        temporary=$(mktemp "${dir}/.${sha}.XXXXXX")
        curl -fsSL --retry 3 --connect-timeout 15 --max-time 300 "${proxy_opt[@]}" \
            "https://codeload.github.com/${repo}/tar.gz/${sha}" -o "${temporary}" \
            || { echo "Failed to download ${action_ref}" >&2; return 1; }
        tar tzf "${temporary}" >/dev/null || { echo "Invalid archive for ${action_ref}" >&2; return 1; }
        chmod 644 "${temporary}"
        mv -f -- "${temporary}" "${out}"
        temporary=""
    done

    echo "Action cache ready at ${ACTION_CACHE_DIR}"
)

# ---- Main ------------------------------------------------------------------
usage() {
    echo "Usage:" >&2
    echo "  sudo $0 deploy        <REGISTRATION_TOKEN>  # populate cache + deploy runners" >&2
    echo "  sudo $0 destroy       <REMOVAL_TOKEN>       # remove all managed runners" >&2
    echo "  sudo $0 refresh-cache                  # (re)populate the action cache only" >&2
    echo "  sudo $0 status                         # service health and recent logs" >&2
    exit 1
}

main() {
    local action="${1:-}" token num instance
    case "${action}" in
        deploy|destroy|refresh-cache|status) ;;
        *) usage ;;
    esac
    exec 9>"${LOCK_FILE}"
    flock -n 9 || { echo "Another runner-manager operation is in progress." >&2; return 1; }
    umask 022
    discover_runners

    case "${action}" in
        deploy)
            [[ $# -ge 2 && -n "$2" ]] || { echo "GitHub registration token required!" >&2; usage; }
            token=$2
            detect_cpu_topology
            configure_docker_proxy
            refresh_action_cache
            for (( num = 1; num <= MAX_RUNNERS; num++ )); do
                deploy_runner "${num}" "${token}"
            done
            echo "Successfully deployed ${MAX_RUNNERS} runners!"
            ;;

        destroy)
            [[ $# -ge 2 && -n "$2" ]] || { echo "GitHub removal token required!" >&2; usage; }
            token=$2
            local failed=()
            for instance in "${STATE_DIR}/instances/"*; do
                [[ -f "${instance}/runner-dir" ]] || continue
                if ! destroy_runner "${instance}" "${token}"; then
                    failed+=("$(basename "${instance}")")
                    echo "Cleanup failed; preserving remaining files and state for ${instance}." >&2
                fi
            done
            if (( ${#failed[@]} )); then
                printf 'Failed to clean up: %s\n' "${failed[*]}" >&2
                return 1
            fi
            echo "All managed runners have been cleaned up!"
            ;;

        refresh-cache)
            refresh_action_cache
            ;;

        status)
            show_status
            ;;
    esac
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
