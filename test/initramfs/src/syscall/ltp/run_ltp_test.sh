#!/bin/sh

# SPDX-License-Identifier: MPL-2.0

LTP_DIR=$(dirname "$0")
TEST_TMP_DIR=${SYSCALL_TEST_WORKDIR:-/tmp}
LOG_FILE="$TEST_TMP_DIR/result.log"
SUMMARY_FILE="$TEST_TMP_DIR/ltp_summary.json"
CASE_STATUS_FILE="$TEST_TMP_DIR/ltp_case_status.tsv"
FAILED_CASES_FILE="$TEST_TMP_DIR/ltp_failed_cases.txt"
BROKEN_CASES_FILE="$TEST_TMP_DIR/ltp_broken_cases.txt"
SKIPPED_CASES_FILE="$TEST_TMP_DIR/ltp_skipped_cases.txt"
REPORTED_CASES_FILE="$TEST_TMP_DIR/ltp_reported_cases.txt"
CASE_LOG_DIR="$TEST_TMP_DIR/ltp_case_logs"
RESULT=0
RUNLTP_EXIT_CODE=0

count_non_empty_lines() {
    file_path="$1"
    if [ ! -s "$file_path" ]; then
        echo 0
        return
    fi

    grep -cve '^[[:space:]]*$' "$file_path"
}

write_json_array() {
    file_path="$1"
    first_item=1
    printf '['
    while IFS= read -r case_name; do
        [ -z "$case_name" ] && continue
        if [ "$first_item" -eq 0 ]; then
            printf ', '
        fi
        printf '"%s"' "$case_name"
        first_item=0
    done < "$file_path"
    printf ']'
}

print_case_bucket() {
    bucket_name="$1"
    case_file="$2"
    case_count=$(count_non_empty_lines "$case_file")
    if [ "$case_count" -eq 0 ]; then
        echo "$bucket_name (0): none"
        return
    fi

    case_list=$(tr '\n' ' ' < "$case_file" | sed 's/[[:space:]]*$//')
    echo "$bucket_name ($case_count): $case_list"
}

rm -f \
    "$LOG_FILE" \
    "$SUMMARY_FILE" \
    "$CASE_STATUS_FILE" \
    "$FAILED_CASES_FILE" \
    "$BROKEN_CASES_FILE" \
    "$SKIPPED_CASES_FILE" \
    "$REPORTED_CASES_FILE"
rm -rf "$CASE_LOG_DIR"
mkdir -p "$CASE_LOG_DIR"
: > "$CASE_STATUS_FILE"
: > "$FAILED_CASES_FILE"
: > "$BROKEN_CASES_FILE"
: > "$SKIPPED_CASES_FILE"

CREATE_ENTRIES=1 "$LTP_DIR/runltp" -f syscalls -Q -p -d "$TEST_TMP_DIR" -l "$LOG_FILE"
RUNLTP_EXIT_CODE=$?
if [ "$RUNLTP_EXIT_CODE" -ne 0 ]; then
    RESULT=1
fi

if [ -f "$LOG_FILE" ]; then
    awk \
        -v failed="$FAILED_CASES_FILE" \
        -v broken="$BROKEN_CASES_FILE" \
        -v skipped="$SKIPPED_CASES_FILE" \
        -v case_status="$CASE_STATUS_FILE" \
        '
            function push_case(bucket, case_name, dedup_key, output_file) {
                dedup_key = bucket ":" case_name
                if (seen[dedup_key]++) {
                    return
                }
                if (bucket == "failed") {
                    output_file = failed
                } else if (bucket == "broken") {
                    output_file = broken
                } else {
                    output_file = skipped
                }
                print case_name >> output_file
            }

            $0 == "<<<test_start>>>" {
                need_tag = 1
                next
            }

            need_tag == 1 {
                if ($0 ~ /^tag=/) {
                    n = split($0, fields, /[[:space:]]+/)
                    for (i = 1; i <= n; i++) {
                        if (fields[i] ~ /^tag=/) {
                            current_case = substr(fields[i], 5)
                            break
                        }
                    }
                }
                need_tag = 0
            }

            {
                case_name = current_case
                status = ""

                if ($0 ~ /^[A-Za-z0-9_.+-]+[[:space:]]+[0-9]+[[:space:]]+T[A-Z]+[[:space:]]*:/) {
                    n = split($0, fields, /[[:space:]]+/)
                    case_name = fields[1]
                    status = fields[3]
                    sub(/:.*/, "", status)
                } else if (index($0, "TFAIL") > 0) {
                    status = "TFAIL"
                } else if (index($0, "TBROK") > 0) {
                    status = "TBROK"
                } else if (index($0, "TSKIP") > 0) {
                    status = "TSKIP"
                } else if (index($0, "TCONF") > 0) {
                    status = "TCONF"
                }

                gsub(/[^A-Za-z0-9_.+-]/, "", case_name)
                bucket = ""
                if (status == "TFAIL") {
                    bucket = "failed"
                } else if (status == "TBROK") {
                    bucket = "broken"
                } else if (status == "TSKIP" || status == "TCONF") {
                    bucket = "skipped"
                }

                if (bucket != "" && case_name != "") {
                    push_case(bucket, case_name)
                    print case_name "\t" bucket "\t" $0 >> case_status
                }
            }
        ' "$LOG_FILE"

    sort -u -o "$FAILED_CASES_FILE" "$FAILED_CASES_FILE"
    sort -u -o "$BROKEN_CASES_FILE" "$BROKEN_CASES_FILE"
    sort -u -o "$SKIPPED_CASES_FILE" "$SKIPPED_CASES_FILE"
    cat "$FAILED_CASES_FILE" "$BROKEN_CASES_FILE" "$SKIPPED_CASES_FILE" | awk 'NF' | sort -u > "$REPORTED_CASES_FILE"

    if ! awk -v outdir="$CASE_LOG_DIR" '
            function sanitize(case_name) {
                safe_name = case_name
                gsub(/[^A-Za-z0-9_.-]/, "_", safe_name)
                return safe_name
            }

            $0 == "<<<test_start>>>" {
                in_case = 1
                case_name = ""
                case_output = $0 "\n"
                next
            }

            in_case == 1 {
                case_output = case_output $0 "\n"
                if (case_name == "" && $0 ~ /^tag=/) {
                    n = split($0, fields, /[[:space:]]+/)
                    for (i = 1; i <= n; i++) {
                        if (fields[i] ~ /^tag=/) {
                            case_name = substr(fields[i], 5)
                            break
                        }
                    }
                }

                if ($0 == "<<<test_end>>>") {
                    if (case_name != "") {
                        print case_output > (outdir "/" sanitize(case_name) ".log")
                        extracted_cases++
                    }
                    in_case = 0
                    case_name = ""
                    case_output = ""
                }
            }

            END {
                if (extracted_cases == 0) {
                    exit 3
                }
            }
        ' "$LOG_FILE"; then
        while IFS= read -r case_name; do
            [ -z "$case_name" ] && continue
            grep -F "$case_name" "$LOG_FILE" > "$CASE_LOG_DIR/$case_name.log" || true
        done < "$REPORTED_CASES_FILE"
    fi
else
    echo "Error: LTP log file not found: $LOG_FILE" >&2
    RESULT=1
fi

FAILED_COUNT=$(count_non_empty_lines "$FAILED_CASES_FILE")
BROKEN_COUNT=$(count_non_empty_lines "$BROKEN_CASES_FILE")
SKIPPED_COUNT=$(count_non_empty_lines "$SKIPPED_CASES_FILE")

{
    printf '{\n'
    printf '  "runltp_exit_code": %s,\n' "$RUNLTP_EXIT_CODE"
    printf '  "log_file": "%s",\n' "$LOG_FILE"
    printf '  "case_status_file": "%s",\n' "$CASE_STATUS_FILE"
    printf '  "case_log_dir": "%s",\n' "$CASE_LOG_DIR"
    printf '  "counts": {\n'
    printf '    "failed": %s,\n' "$FAILED_COUNT"
    printf '    "broken": %s,\n' "$BROKEN_COUNT"
    printf '    "skipped": %s\n' "$SKIPPED_COUNT"
    printf '  },\n'
    printf '  "failed_cases": '
    write_json_array "$FAILED_CASES_FILE"
    printf ',\n'
    printf '  "broken_cases": '
    write_json_array "$BROKEN_CASES_FILE"
    printf ',\n'
    printf '  "skipped_cases": '
    write_json_array "$SKIPPED_CASES_FILE"
    printf '\n'
    printf '}\n'
} > "$SUMMARY_FILE"

if [ -f "$LOG_FILE" ]; then
    cat "$LOG_FILE"
    if grep -q "Total Failures:" "$LOG_FILE" && ! grep -Eq "Total Failures:[[:space:]]*0" "$LOG_FILE"; then
        RESULT=1
    fi
fi

echo "LTP machine summary: $SUMMARY_FILE"
echo "LTP per-case status: $CASE_STATUS_FILE"
echo "LTP per-case logs dir: $CASE_LOG_DIR"
print_case_bucket "Failed cases" "$FAILED_CASES_FILE"
print_case_bucket "Broken cases" "$BROKEN_CASES_FILE"
print_case_bucket "Skipped cases" "$SKIPPED_CASES_FILE"

if [ "$FAILED_COUNT" -gt 0 ] || [ "$BROKEN_COUNT" -gt 0 ]; then
    RESULT=1
fi

exit $RESULT
