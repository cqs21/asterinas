#!/bin/sh

# SPDX-License-Identifier: MPL-2.0

LTP_DIR=$(dirname "$0")
TEST_TMP_DIR=${CONFORMANCE_TEST_WORKDIR:-/tmp}
LOG_FILE=$TEST_TMP_DIR/result.log
RESULT=0

# Some test cases require a block device. Select the first unused `/dev/vd*`
# device that is not mounted and export it as `LTP_DEV`.
if [ -z "${LTP_DEV}" ]; then
    for candidate in /dev/vd[a-z]; do
        [ -b "$candidate" ] || continue
        if ! grep -q "^${candidate}[0-9]* " /proc/mounts; then
            LTP_DEV="$candidate"
            break
        fi
    done
fi
export LTP_DEV

rm -f $LOG_FILE
CREATE_ENTRIES=1 $LTP_DIR/runltp -f syscalls -Q -p -d $TEST_TMP_DIR -l $LOG_FILE
if [ $? -ne 0 ]; then
    RESULT=1
fi

cat $LOG_FILE
if ! grep -q "Total Failures: 0" $LOG_FILE; then
    RESULT=1
fi

exit $RESULT
