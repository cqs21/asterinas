#!/bin/bash

# SPDX-License-Identifier: MPL-2.0

set -euo pipefail

print_help() {
    cat <<'EOF'
Usage: select_matrix_items.sh [options]

Selects a filtered matrix based on the current GitHub event and changed files.

Options:
  --event-name <name>          GitHub event name, such as pull_request or push.
  --matrix-json <json>         JSON array of matrix objects.
  --matrix-name-key <key>      Field used to match suite names. Defaults to `name`.
  --pr-base-sha <sha>          Base SHA for pull_request events.
  --pr-head-sha <sha>          Head SHA for pull_request events.
  --push-before-sha <sha>      Previous SHA for push events.
  --push-head-sha <sha>        Current SHA for push events.
  --always-run-on <event>      Event that always keeps the full matrix. Repeatable.
  --shared-file <path>         Exact file path that keeps the full matrix. Repeatable.
  --shared-prefix <path>       Path prefix that keeps the full matrix. Repeatable.
  --suite-prefix <path>        Base path whose '<path>/<matrix-name>/**' changes
                               select a single matrix item. Repeatable.
  --changed-file <path>        Overrides git diff input with an explicit changed file.
                               Repeatable.
  -h, --help                   Prints this help message.

Outputs:
  Writes `run`, `reason`, `matrix`, and `matched_file` to `GITHUB_OUTPUT` when
  available. Also prints a short summary to stdout.
EOF
}

append_output() {
    local key="$1"
    local value="$2"

    if [ -n "${GITHUB_OUTPUT:-}" ]; then
        printf '%s=%s\n' "${key}" "${value}" >> "${GITHUB_OUTPUT}"
    fi
}

finish() {
    local run="$1"
    local reason="$2"
    local matrix_json="$3"
    local matched_file="${4:-}"

    append_output "run" "${run}"
    append_output "reason" "${reason}"
    append_output "matrix" "${matrix_json}"
    append_output "matched_file" "${matched_file}"
    printf 'run=%s reason=%s matched_file=%s matrix=%s\n' \
        "${run}" "${reason}" "${matched_file}" "${matrix_json}"
    exit 0
}

normalize_prefix() {
    local prefix="$1"
    prefix="${prefix%/}"
    printf '%s\n' "${prefix}"
}

matches_event() {
    local current_event="$1"
    shift

    local candidate
    for candidate in "$@"; do
        if [ "${candidate}" = "${current_event}" ]; then
            return 0
        fi
    done

    return 1
}

collect_changed_files_from_git() {
    if [ "${event_name}" = "pull_request" ]; then
        if [ -z "${pr_base_sha}" ] || [ -z "${pr_head_sha}" ]; then
            echo "Error: pull_request events require both --pr-base-sha and --pr-head-sha."
            exit 1
        fi
        git diff --name-only "${pr_base_sha}" "${pr_head_sha}"
        return
    fi

    if [ "${event_name}" = "push" ]; then
        if [ -z "${push_head_sha}" ]; then
            echo "Error: push events require --push-head-sha."
            exit 1
        fi

        if [ "${push_before_sha}" = "0000000000000000000000000000000000000000" ]; then
            git diff-tree --no-commit-id --name-only -r "${push_head_sha}"
        else
            if [ -z "${push_before_sha}" ]; then
                echo "Error: push events require --push-before-sha."
                exit 1
            fi
            git diff --name-only "${push_before_sha}" "${push_head_sha}"
        fi
        return
    fi
}

select_matrix_by_names() {
    local affected_names_json="$1"

    jq -c \
        --arg name_key "${matrix_name_key}" \
        --argjson affected_names "${affected_names_json}" \
        '[.[] | select((.[$name_key]) as $name | $affected_names | index($name))]' \
        <<<"${matrix_json}"
}

event_name=""
matrix_json=""
matrix_name_key="name"
pr_base_sha=""
pr_head_sha=""
push_before_sha=""
push_head_sha=""
always_run_events=()
shared_prefixes=()
shared_files=()
suite_prefixes=()
changed_files_override=()

while [ "$#" -gt 0 ]; do
    case "$1" in
        --event-name)
            event_name="$2"
            shift 2
            ;;
        --matrix-json)
            matrix_json="$2"
            shift 2
            ;;
        --matrix-name-key)
            matrix_name_key="$2"
            shift 2
            ;;
        --pr-base-sha)
            pr_base_sha="$2"
            shift 2
            ;;
        --pr-head-sha)
            pr_head_sha="$2"
            shift 2
            ;;
        --push-before-sha)
            push_before_sha="$2"
            shift 2
            ;;
        --push-head-sha)
            push_head_sha="$2"
            shift 2
            ;;
        --always-run-on)
            always_run_events+=("$2")
            shift 2
            ;;
        --shared-file)
            shared_files+=("$2")
            shift 2
            ;;
        --shared-prefix)
            shared_prefixes+=("$(normalize_prefix "$2")")
            shift 2
            ;;
        --suite-prefix)
            suite_prefixes+=("$(normalize_prefix "$2")")
            shift 2
            ;;
        --changed-file)
            changed_files_override+=("$2")
            shift 2
            ;;
        -h|--help)
            print_help
            exit 0
            ;;
        *)
            echo "Error: unknown option '$1'."
            print_help
            exit 1
            ;;
    esac
done

if [ -z "${event_name}" ] || [ -z "${matrix_json}" ]; then
    echo "Error: both --event-name and --matrix-json are required."
    print_help
    exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="${script_dir}/../.."
cd "${repo_root}"

matrix_json="$(jq -c '.' <<<"${matrix_json}")"

if matches_event "${event_name}" "${always_run_events[@]:-}"; then
    finish "true" "always-run-event" "${matrix_json}"
fi

changed_files=()
if [ "${#changed_files_override[@]}" -gt 0 ]; then
    changed_files=("${changed_files_override[@]}")
else
    while IFS= read -r changed_file; do
        [ -n "${changed_file}" ] || continue
        changed_files+=("${changed_file}")
    done < <(collect_changed_files_from_git)
fi

if [ "${#changed_files[@]}" -eq 0 ]; then
    finish "false" "no-changed-files" '[]'
fi

affected_names_json='[]'
changed_file=""
prefix=""
exact_file=""

for changed_file in "${changed_files[@]}"; do
    for exact_file in "${shared_files[@]:-}"; do
        if [ "${changed_file}" = "${exact_file}" ]; then
            finish "true" "shared-file-match" "${matrix_json}" "${changed_file}"
        fi
    done

    for prefix in "${shared_prefixes[@]:-}"; do
        if [[ "${changed_file}" == "${prefix}/"* ]]; then
            finish "true" "shared-prefix-match" "${matrix_json}" "${changed_file}"
        fi
    done

    for prefix in "${suite_prefixes[@]:-}"; do
        if [[ "${changed_file}" != "${prefix}/"* ]]; then
            continue
        fi

        suite_name="${changed_file#${prefix}/}"
        suite_name="${suite_name%%/*}"
        affected_names_json="$(
            jq -c \
                --arg suite_name "${suite_name}" \
                'if index($suite_name) then . else . + [$suite_name] end' \
                <<<"${affected_names_json}"
        )"
    done
done

selected_matrix="$(select_matrix_by_names "${affected_names_json}")"
if [ "${selected_matrix}" = "[]" ]; then
    finish "false" "no-matching-change" '[]'
fi

finish "true" "suite-prefix-match" "${selected_matrix}"
