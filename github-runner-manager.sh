#!/bin/bash
# github-runner-manager.sh
# Usage:
#   Deploy all runners:  sudo ./github-runner-manager.sh deploy  <GITHUB_TOKEN>
#   Destroy all runners: sudo ./github-runner-manager.sh destroy <GITHUB_TOKEN>
#
# The token is a short-lived GitHub Actions registration/removal token,
# obtained from the repo's Settings > Actions > Runners page (or via the API).

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
RUNNER_TARBALL="actions-runner-linux-x64-2.334.0.tar.gz"
GITHUB_URL="https://github.com/asterinas/asterinas"
RUNNER_LABEL="x86"
RUNNER_NAME="${RUNNER_LABEL}-runner"
MAX_RUNNERS=4                     # Number of runners to deploy
PHYSICAL_CORES_PER_RUNNER=6       # Physical cores dedicated to each runner
                                  # 4 runners x 6 = 24 physical cores

# HTTP(S) proxy the runner uses to reach GitHub (github.com, codeload.github.com,
# api.github.com, ...).
RUNNER_HTTPS_PROXY="http://127.0.0.1:7890"
RUNNER_NO_PROXY="localhost,127.0.0.1"

# Action-archive cache. The runner checks this dir before downloading an action
# from codeload.github.com: a hit is copied from local disk (zero network).
# Layout it looks up (from the runner source): <dir>/<owner>_<repo>/<sha>.tar.gz
ACTION_CACHE_DIR="${RUNNER_BASE_DIR}/action-cache"
# Actions to pre-populate. Pin to a SHA (owner/repo@<sha>) for a deterministic
# cache hit; a tag/branch is re-resolved to a SHA each refresh and may miss later.
# These are the actions the self-hosted benchmark jobs pull from codeload:
# checkout (both jobs) and github-action-benchmark (referenced by the local
# composite ./.github/actions/benchmark, downloaded during "Prepare all required
# actions"). The artifact actions were removed from the composite in favor of a
# shared host directory, so they no longer need caching.
ACTION_CACHE_LIST=(
    "actions/checkout@v4"
    "asterinas/github-action-benchmark@v5"
)

# ---- CPU topology (auto-detected) -----------------------------------------
# Build an ordered list of physical cores, each entry being that core's full
# set of logical CPUs (its hyperthread siblings), e.g. "0,32". Reading the
# real sibling list avoids assuming any particular CPU numbering scheme.
declare -a PHYS_CORES=()
declare -A _seen=()
for _cpu in $(ls -d /sys/devices/system/cpu/cpu[0-9]* 2>/dev/null | sort -V); do
    _siblings_file="${_cpu}/topology/thread_siblings_list"
    [[ -r "${_siblings_file}" ]] || continue
    _siblings=$(<"${_siblings_file}")
    if [[ -z "${_seen[${_siblings}]:-}" ]]; then
        _seen[${_siblings}]=1
        PHYS_CORES+=("${_siblings}")
    fi
done

if (( ${#PHYS_CORES[@]} == 0 )); then
    echo "Error: failed to detect CPU topology" >&2
    exit 1
fi

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

    local spec=""
    for (( c = start; c <= end; c++ )); do
        spec+="${PHYS_CORES[c]},"
    done
    echo "${spec%,}"
}

# ---- Deploy ----------------------------------------------------------------
deploy_runner() {
    local num=$1
    local token=$2
    local runner_dir="${RUNNER_BASE_DIR}/${RUNNER_PREFIX}${num}/actions-runner"
    local runner_name="${RUNNER_NAME}-${num}"

    echo "Deploying ${runner_name}..."

    # Fail fast, before touching GitHub or systemd, if this runner can't be
    # pinned to a valid set of cores.
    local core_range
    core_range=$(cpu_range_for_runner "${num}")

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

    # The runner loads this .env on startup. Point it at the proxy so action
    # archives (codeload.github.com) download reliably from the service context.
    {
        if [[ -n "${RUNNER_HTTPS_PROXY}" ]]; then
            echo "https_proxy=${RUNNER_HTTPS_PROXY}"
            echo "http_proxy=${RUNNER_HTTPS_PROXY}"
            echo "no_proxy=${RUNNER_NO_PROXY}"
        fi
        if [[ -n "${ACTION_CACHE_DIR}" ]]; then
            echo "ACTIONS_RUNNER_ACTION_ARCHIVE_CACHE=${ACTION_CACHE_DIR}"
        fi
    } > "${runner_dir}/.env"

    # config.sh and the runner service must run as an unprivileged user, so
    # hand ownership of the whole runner tree to SVC_USER before configuring.
    chown -R "${SVC_USER}:${SVC_USER}" "${RUNNER_BASE_DIR}/${RUNNER_PREFIX}${num}"

    # Configure runner as SVC_USER (config.sh refuses to run as root).
    # --replace makes re-deploys idempotent. svc.sh install still runs as root.
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
        ./svc.sh install "${SVC_USER}"
    )

    # svc.sh records the exact systemd unit name (e.g.
    # actions.runner.<owner>-<repo>.<runner_name>.service) in this file. Read it
    # instead of reconstructing the name, so changing GITHUB_URL just works.
    local svc_name_file="${runner_dir}/.service"
    if [[ ! -f "${svc_name_file}" ]]; then
        echo "Error: ${svc_name_file} not found; svc.sh install may have failed" >&2
        return 1
    fi
    local service_file="/etc/systemd/system/$(<"${svc_name_file}")"
    if [[ ! -f "${service_file}" ]]; then
        echo "Error: Service file not found at ${service_file}" >&2
        return 1
    fi

    echo "Binding ${runner_name} to CPU(s) ${core_range}..."
    sed -i \
        "s|^ExecStart=.*|ExecStart=/usr/bin/taskset -c ${core_range} ${runner_dir}/runsvc.sh|" \
        "${service_file}"

    systemctl daemon-reload
    systemctl restart "$(basename "${service_file}")"
}

# ---- Destroy ---------------------------------------------------------------
# Best-effort cleanup: keep going even if an individual step fails, so one
# broken runner doesn't block tearing down the rest.
destroy_runner() {
    local num=$1
    local token=$2
    local runner_dir="${RUNNER_BASE_DIR}/${RUNNER_PREFIX}${num}/actions-runner"
    local runner_name="${RUNNER_NAME}-${num}"

    echo "Destroying ${runner_name}..."

    if [[ -f "${runner_dir}/svc.sh" ]]; then
        (
            cd "${runner_dir}"
            ./svc.sh stop      || true
            ./svc.sh uninstall || true
            sudo -u "${SVC_USER}" ./config.sh remove --token "${token}" || true
        )
    fi

    rm -rf "${RUNNER_BASE_DIR}/${RUNNER_PREFIX}${num}"
}

# ---- Action archive cache --------------------------------------------------
# Pre-download each action's source tarball into ACTION_CACHE_DIR using the
# proxy (which works interactively), so the runner never has to fetch it during
# a job. Safe to re-run; existing archives are skipped. Optional GH_TOKEN raises
# the api.github.com rate limit when resolving tags to SHAs.
refresh_action_cache() {
    [[ -n "${ACTION_CACHE_DIR}" ]] || { echo "ACTION_CACHE_DIR unset" >&2; return 1; }

    local proxy_opt=()
    [[ -n "${RUNNER_HTTPS_PROXY}" ]] && proxy_opt=(--proxy "${RUNNER_HTTPS_PROXY}")
    local auth=()
    [[ -n "${GH_TOKEN:-}" ]] && auth=(-H "Authorization: Bearer ${GH_TOKEN}")

    for a in "${ACTION_CACHE_LIST[@]}"; do
        local repo="${a%@*}" ref="${a##*@}"
        # Resolve tag/branch to the commit SHA the runner keys the cache on.
        local sha
        sha=$(curl -fsSL --max-time 60 "${proxy_opt[@]}" "${auth[@]}" \
            -H "Accept: application/vnd.github.sha" \
            "https://api.github.com/repos/${repo}/commits/${ref}") \
            || { echo "Failed to resolve ${a}" >&2; return 1; }

        local dir="${ACTION_CACHE_DIR}/${repo/\//_}"
        local out="${dir}/${sha}.tar.gz"
        mkdir -p "${dir}"
        if [[ -f "${out}" ]]; then
            echo "cached  ${repo}@${sha}"
            continue
        fi
        echo "fetch   ${repo}@${ref} -> ${sha}"
        curl -fsSL --max-time 300 "${proxy_opt[@]}" "${auth[@]}" \
            "https://codeload.github.com/${repo}/tar.gz/${sha}" -o "${out}" \
            || { echo "Failed to download ${a}" >&2; rm -f "${out}"; return 1; }
    done

    chown -R "${SVC_USER}:${SVC_USER}" "${ACTION_CACHE_DIR}"
    echo "Action cache ready at ${ACTION_CACHE_DIR}"
}

# ---- Main ------------------------------------------------------------------
action="${1:-}"

usage() {
    echo "Usage:" >&2
    echo "  sudo $0 deploy        <GITHUB_TOKEN>   # populate cache + deploy runners" >&2
    echo "  sudo $0 destroy       <GITHUB_TOKEN>   # remove all runners" >&2
    echo "  sudo $0 refresh-cache                  # (re)populate the action cache only" >&2
    exit 1
}

case "${action}" in
    deploy)
        [[ $# -ge 2 && -n "$2" ]] || { echo "GitHub registration token required!" >&2; usage; }
        token=$2
        refresh_action_cache
        for i in $(seq 1 "${MAX_RUNNERS}"); do
            deploy_runner "${i}" "${token}"
        done
        echo "Successfully deployed ${MAX_RUNNERS} runners!"
        systemctl list-units | grep actions.runner || true
        ;;

    destroy)
        [[ $# -ge 2 && -n "$2" ]] || { echo "GitHub registration token required!" >&2; usage; }
        token=$2
        for i in $(seq 1 "${MAX_RUNNERS}"); do
            destroy_runner "${i}" "${token}"
        done
        echo "All runners have been cleaned up!"
        ;;

    refresh-cache)
        refresh_action_cache
        ;;

    *)
        usage
        ;;
esac
