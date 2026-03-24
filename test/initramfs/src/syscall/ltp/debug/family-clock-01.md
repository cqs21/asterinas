# Clock Family Batch 01

## Goal

Re-enable a first `clock_*` batch that already had partial syscall support and
should pass consistently across `/tmp`, `/ext2`, and `/exfat`.

Enabled cases in this batch:

- `clock_getres01`
- `clock_gettime01`
- `clock_nanosleep01`
- `clock_nanosleep02`

## Problem Cause

This batch initially had three independent blockers:

1. Disabled-case debugging did not work through the normal Nix initramfs build
   path because `LTP_CASES`, `LTP_CASE_FILE`, and `EXTRA_BLOCKLISTS_DIRS` were
   not forwarded into the packaged syscall test environment.
2. `clock_getres01` failed because Asterinas had no `clock_getres` syscall
   implementation or syscall-table entry.
3. `clock_nanosleep01` failed because `CLOCK_THREAD_CPUTIME_ID` returned
   `EINVAL` instead of Linux-compatible `EOPNOTSUPP`.
4. `clock_nanosleep02` failed after adding `clock_getres` because the reported
   clock resolution was unrealistically optimistic (`1ns`), which made LTP use
   thresholds that were too strict for the kernel's actual timer granularity.

`clock_gettime02` was also investigated but remains out of this batch because
it currently `TBROK`s on a harness/environment assumption: LTP wants a parsable
kernel `.config`.

## Solution

### Harness changes used by this batch

- Forwarded `LTP_CASES`, `LTP_CASE_FILE`, and `EXTRA_BLOCKLISTS_DIRS` through
  the Nix-based initramfs build path so targeted disabled-case debugging works
  with `make run_kernel`.
- Relaxed LTP packaging semantics for explicit case selection so a selected
  case can be debugged even if it is still commented out in `testcases/all.txt`
  or normally excluded by persistent blocklists.

### Kernel fixes used by this batch

- Added `clock_getres` support to the syscall dispatch tables and implemented
  `sys_clock_getres`.
- Extended the clock ID set to include `CLOCK_REALTIME_ALARM` and
  `CLOCK_BOOTTIME_ALARM` for `clock_getres`.
- Returned `EOPNOTSUPP` for `clock_nanosleep(CLOCK_THREAD_CPUTIME_ID, ...)`
  instead of `EINVAL`.
- Reported `clock_getres()` using the kernel's effective jiffy-based timer
  granularity (`1ms` with `TIMER_FREQ=1000`) instead of an artificial `1ns`.

### Validation results

The batch passed on all three required targets:

- `/tmp`
- `/ext2`
- `/exfat`

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=clock_getres01,clock_gettime01,clock_nanosleep01,clock_nanosleep02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=clock_getres01,clock_gettime01,clock_nanosleep01,clock_nanosleep02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=clock_getres01,clock_gettime01,clock_nanosleep01,clock_nanosleep02
```

## Follow-up

- `clock_gettime02` is still blocked by a harness/environment issue
  (`Cannot parse kernel .config`) and should be tracked separately.
- `clock_settime01` and `clock_adjtime01` remain feature work rather than
  compatibility nits; they were not mixed into this batch.
