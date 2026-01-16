#!/bin/sh

# SPDX-License-Identifier: MPL-2.0

BASE_DIR=@kselftest@

TESTS="$BASE_DIR"/kselftest-list.txt
if [ ! -r "$TESTS" ] ; then
	echo "$0: Could not find list of tests to run ($TESTS)" >&2
	available=""
else
	available="$(cat "$TESTS")"
fi

blocklists=""
BLOCKLISTS_DIR="$(dirname $0)/blocklists"
for blocklist_file in "$BLOCKLISTS_DIR"/*; do
    while IFS= read -r line || [ -n "$line" ]; do
        line=$(echo "$line" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
		case "$line" in
			"#"*)
				continue ;;
			*:*)
				collection=$(echo "$line" | cut -d: -f1)
                test=$(echo "$line" | cut -d: -f2)
				if [ "$test" = "*" ]; then
					matched_lines=$(echo "$available" | grep "^$collection:")
					blocklists="$blocklists $matched_lines"
				else
					blocklists="$blocklists $line"
				fi
                ;;
            *)
                echo "Warning: Invalid format in blocklist: $line" >&2
                continue ;;
		esac
	done < "$blocklist_file"
done
blocklists="$(echo "$blocklists" | tr ' ' '\n')"

testcases="$(echo "$available" | grep -vxF "$blocklists" | grep -v '^$')"

collections=$(echo "$testcases" | cut -d: -f1 | sort | uniq)
for collection in $collections ; do
	echo "Running tests in $collection"
	tests=$(echo "$testcases" | grep "^$collection:" | cut -d: -f2)
	for test in $tests ; do
		(cd "$BASE_DIR"/$collection && ./$test)
	done
done
