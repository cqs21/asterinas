# LTP Family-Based Re-enable Triage Sheet (Task 2.1)

This sheet groups LTP backlog items by subsystem family (not alphabetically),
so contributors can pick small, coherent re-enable batches.

## Data source snapshot

- `testcases/all.txt`
- `testcases/blocked/ext2.txt`
- `testcases/blocked/exfat.txt`

Interpretation used in this snapshot:

- `all.txt` lines matching `^# <case_name>$` are treated as `disabled`.
- `all.txt` uncommented non-empty lines are treated as `enabled`.
- Filesystem blocklists are treated as known FS-specific exclusions.
- Grouping is prefix-based and aligned to the initial family buckets in
  `TASKS.md`.

## Backlog size by family

| Family | Disabled in `all.txt` | `ext2` blocked | `exfat` blocked |
| --- | ---: | ---: | ---: |
| File descriptor and eventing | 77 | 3 | 8 |
| Filesystem and VFS | 166 | 25 | 46 |
| Process and scheduling | 40 | 6 | 0 |
| Signals and timers | 37 | 10 | 0 |
| Memory and mapping | 45 | 0 | 8 |
| Credentials and security | 68 | 0 | 4 |
| Networking and sockets | 43 | 0 | 1 |
| Large missing features | 190 | 0 | 0 |

Notes:

- There are also `345` disabled cases outside the initial family buckets
  (`other-unclassified`), which should be split into additional families
  during later backlog grooming.

## Family details

### 1) File descriptor and eventing

Primary disabled prefixes:

- `fcntl` (50), `pipe` (7), `eventfd` (6), `dup` (4), `epoll_*` (7), `select` (2), `poll` (1).

Current FS-specific blocklists:

- `ext2`: `dup05`, `select01`, `select03`.
- `exfat`: `dup05`, `dup203`, `dup205`, `pipeio_1`, `pipeio_2`, `pipeio_5`, `pipeio_7`, `select01`.

Suggested near-term batches:

- `fcntl07`, `fcntl07_64`, `fcntl11`, `fcntl11_64`, `fcntl12`, `fcntl12_64`.
- `epoll_ctl02`, `epoll_ctl04`, `epoll_wait02`, `epoll_wait05`, `epoll_pwait03`.

### 2) Filesystem and VFS

Primary disabled prefixes:

- `rename` (13), `statx` (10), `open` (9), `statmount` (9), `sendfile` (8),
  `chown` (8), `openat` (6), `mknod` (7), `utime` (6), `statfs` (6), `creat` (6).

Current FS-specific blocklists:

- `ext2`: `getcwd02`, `linkat01`, `mknod01`, `rename14`, `sendfile02`, `sendfile02_64`,
  `sendfile05`, `sendfile05_64`, `sendfile06`, `sendfile06_64`, `sendfile08`,
  `sendfile08_64`, `stat02`, `stat02_64`, `symlink02`, `symlink04`, `symlinkat01`,
  `truncate02`, `truncate02_64`, `unlink01`, `unlink05`, `unlinkat01`, `utime07`,
  `statx02`, `statx03`.
- `exfat`: `chmod01`, `chmod01A`, `chmod08`, `chown05`, `creat03`, `fchmod01`,
  `fchmodat01`, `fchown05`, `fchownat01`, `fchownat02`, `fstat02`, `fstat02_64`,
  `ftruncate01`, `ftruncate01_64`, `getcwd03`, `lchown01`, `link01`, `link02`,
  `link05`, `linkat01`, `lstat01`, `lstat01_64`, `mkdir04`, `mknod01`, `open07`,
  `open15`, `openat01`, `rename01A`, `sendfile02`, `sendfile02_64`, `sendfile06`,
  `sendfile06_64`, `sendfile08`, `sendfile08_64`, `stat02`, `stat02_64`, `symlink02`,
  `symlink04`, `symlinkat01`, `truncate02`, `truncate02_64`, `unlink01`, `unlink05`,
  `unlinkat01`, `utime07`, `statx02`.

Suggested near-term batches:

- `statx01`, `statx04`, `statx05`, `statx06`, `statx07`.
- `rename01`, `rename03`, `rename04`, `rename05`, `rename06`.

### 3) Process and scheduling

Primary disabled prefixes:

- `clone` (5), `waitpid` (5), `wait*` (3+), `sched_*` (17 total), `pidfd_send_signal` (3), `pidfd_getfd` (1), `ioprio_set` (1).

Current FS-specific blocklists:

- `ext2`: `waitpid06`, `waitpid07`, `waitpid09`, `waitpid11`, `waitpid12`, `waitid04`.
- `exfat`: none.

Suggested near-term batches:

- `sched_getscheduler01`, `sched_getscheduler02`, `sched_setscheduler01`, `sched_setscheduler02`, `sched_setscheduler03`.
- `sched_setparam02`, `sched_setparam04`, `sched_setparam05`, `sched_getparam03`.

### 4) Signals and timers

Primary disabled prefixes:

- `clock_*` (15), `timer*`/`timerfd*` (13), `rt_sig*` (3), `sig*` (5), `alarm` (1), `setitimer` (1).

Current FS-specific blocklists:

- `ext2`: `rt_sigaction01`, `rt_sigaction02`, `rt_sigaction03`, `rt_sigprocmask01`,
  `rt_sigprocmask02`, `sigaltstack02`, `signalfd02`, `signalfd4_01`, `signalfd4_02`,
  `sigrelse01`.
- `exfat`: none.

Suggested near-term batches:

- `clock_getres01`, `clock_gettime01`, `clock_gettime02`, `clock_nanosleep01`, `clock_nanosleep02`.
- `clock_settime01`, `clock_settime02`, `clock_adjtime01`, `clock_adjtime02`.

### 5) Memory and mapping

Primary disabled prefixes:

- `mmap` (10), `madvise` (11), `mlock*` (13), `mremap` (2), `mprotect` (2), `memfd_create` (4), `brk` (2).

Current FS-specific blocklists:

- `ext2`: none.
- `exfat`: `mmap06`, `mmap19`, `mprotect02`, `mprotect03`, `msync01`, `msync02`, `munmap01`, `munmap02`.

Suggested near-term batches:

- `mprotect01`, `mprotect04`, `mremap04`, `mremap06`, `munlock01`.
- `madvise01`, `madvise02`, `madvise03`, `madvise05`, `madvise06`.

### 6) Credentials and security

Primary disabled prefixes:

- `setreuid` (7), `setresuid` (6), `setresgid` (5), `setregid` (4), `prctl` (7), `unshare` (4), `setns` (2), `cap*` (3).

Current FS-specific blocklists:

- `ext2`: none.
- `exfat`: `setfsuid04`, `setresuid04`, `setreuid07`, `setuid04`.

Suggested near-term batches:

- `capget01`, `capset01`, `capset02`, `prctl02`, `prctl05`.
- `setns01`, `setns02`, `unshare01`, `unshare02`, `unshare03`.

### 7) Networking and sockets

Primary disabled prefixes:

- `setsockopt` (9), `bind` (4), `accept` (4), `sendmsg` (3), `recvmsg` (3), `socketcall` (3), plus `connect`/`shutdown`/`getsockopt`.

Current FS-specific blocklists:

- `ext2`: none.
- `exfat`: `bind04`.

Suggested near-term batches:

- `accept01`, `accept02`, `accept03`, `accept4_01`, `connect01`.
- `setsockopt01`, `setsockopt02`, `setsockopt04`, `setsockopt05`, `setsockopt06`.

### 8) Large missing features

Primary disabled prefixes:

- `fanotify` (24), `inotify*` (14), `ioctl*` (36), `keyctl` (9), `landlock` (10),
  `bpf*` (8), SysV IPC (`msg*`, `sem*`, `shm*`), `quotactl` (9), `fsopen/fsconfig/fsmount/fspick`, `add_key/request_key`.

Current FS-specific blocklists:

- `ext2`: none.
- `exfat`: none.

Planning note:

- Keep this bucket as separate feature projects, not mixed with compatibility
  nits from other families.

## How to refresh this sheet

Use the same parsing assumptions and regenerate counts from:

- `test/initramfs/src/syscall/ltp/testcases/all.txt`
- `test/initramfs/src/syscall/ltp/testcases/blocked/ext2.txt`
- `test/initramfs/src/syscall/ltp/testcases/blocked/exfat.txt`

When refreshing, keep family names stable so task ownership and progress
tracking remain comparable across updates.
