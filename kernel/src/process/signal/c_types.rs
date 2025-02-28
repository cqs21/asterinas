// SPDX-License-Identifier: MPL-2.0

#![expect(dead_code)]
#![expect(non_camel_case_types)]

use core::mem::{self, size_of};

use aster_util::{read_union_fields, union_read_ptr::UnionReadPtr};

use super::sig_num::SigNum;
use crate::{
    arch::cpu::GpRegs,
    prelude::*,
    process::{Pid, Uid},
};

pub type sigset_t = u64;
// FIXME: this type should be put at suitable place
pub type clock_t = i64;

#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct sigaction_t {
    pub handler_ptr: Vaddr,
    pub flags: u32,
    pub restorer_ptr: Vaddr,
    pub mask: sigset_t,
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct siginfo_t {
    pub signo: i32,
    pub errno: i32,
    pub code: i32,
    _padding: i32,
    /// siginfo_fields should be a union type ( See occlum definition ). But union type have unsafe interfaces.
    /// Here we use a simple byte array.
    pub fields: siginfo_fields_t,
}

impl siginfo_t {
    pub fn new(num: SigNum, code: i32) -> Self {
        siginfo_t {
            signo: num.as_u8() as i32,
            errno: 0,
            code: code,
            _padding: 0,
            fields: siginfo_fields_t::zero_fields(),
        }
    }
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub union siginfo_fields_t {
    bytes: [u8; 128 - mem::size_of::<i32>() * 4],
    pub kill: siginfo_kill_t,
    pub timer: siginfo_timer_t,
    pub rt: siginfo_rt_t,
    pub sigchld: siginfo_sigchld_t,
    pub sigfault: siginfo_sigfault_t,
    pub sigpoll: siginfo_sigpoll_t,
    pub sigsys: siginfo_sigsys_t,
}

impl siginfo_fields_t {
    fn zero_fields() -> Self {
        Self {
            bytes: [0; 128 - mem::size_of::<i32>() * 4],
        }
    }
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct siginfo_kill_t {
    pub pid: Pid, // sender's pid
    pub uid: Uid, // sender's uid
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct siginfo_timer_t {
    pub tid: i32,        // timer id
    pub overrun: i32,    // overrun count
    pub value: sigval_t, // Additional signal data, user defined.
    pub private: i32,    // Not used by the kernel. Historic leftover. Always 0.
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct siginfo_rt_t {
    pub pid: Pid, // sender's pid
    pub uid: Uid, // sender's uid
    pub value: sigval_t,
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub union sigval_t {
    pub sigval_int: i32,
    pub sigval_ptr: Vaddr, //*mut c_void
}

impl sigval_t {
    pub fn read_int(&self) -> i32 {
        read_union_fields!(self.sigval_int)
    }

    pub fn read_ptr(&self) -> Vaddr {
        read_union_fields!(self.sigval_ptr)
    }
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct siginfo_sigchld_t {
    pub pid: Pid,    // sender's pid
    pub uid: Uid,    // sender's uid
    pub status: i32, // exit code
    pub utime: u64,  // user time consumed
    pub stime: u64,  // system time consumed
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct siginfo_sigfault_t {
    pub addr: Vaddr, // faulting insn/memory ref
    pub extra: sigfault_extra_t,
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub union sigfault_extra_t {
    // used on alpha and sparc
    pub trapno: i32, // TRAP # which caused the signal
    // used when si_code=BUS_MCEERR_AR or si_code=BUS_MCEERR_AO
    pub addr_lsb: i16, // LSB of the reported address
    // used when si_code=SEGV_BNDERR
    pub addr_bnd: sigfault_addr_bnd_t,
    // used when si_code=SEGV_PKUERR
    pub addr_pkey: sigfault_addr_pkey_t,
    // used when si_code=TRAP_PERF
    pub perf: sigfault_perf_t,
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct sigfault_addr_bnd_t {
    pub dummy: [u8; 8],
    pub lower: Vaddr,
    pub upper: Vaddr,
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct sigfault_addr_pkey_t {
    pub dummy: [u8; 8],
    pub pkey: u32,
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct sigfault_perf_t {
    pub data: u64,
    pub type_: u32,
    pub flags: u32,
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct siginfo_sigpoll_t {
    pub band: u64, // POLL_IN, POLL_OUT, POLL_MSG
    pub fd: i32,   // file descriptor
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct siginfo_sigsys_t {
    pub call_addr: Vaddr, // calling user insn
    pub syscall: i32,     // triggering system call number
    pub arch: u32,        // AUDIT_ARCH_* of syscall
}

#[derive(Clone, Copy, Debug, Pod)]
#[repr(C)]
pub struct ucontext_t {
    pub uc_flags: u64,
    pub uc_link: Vaddr, // *mut ucontext_t
    pub uc_stack: stack_t,
    pub uc_mcontext: mcontext_t,
    pub uc_sigmask: sigset_t,
    pub fpregs: [u8; 64 * 8], //fxsave structure
}

impl Default for ucontext_t {
    fn default() -> Self {
        Self {
            uc_flags: Default::default(),
            uc_link: Default::default(),
            uc_stack: Default::default(),
            uc_mcontext: Default::default(),
            uc_sigmask: Default::default(),
            fpregs: [0u8; 64 * 8],
        }
    }
}

pub type stack_t = sigaltstack_t;

#[derive(Debug, Clone, Copy, Pod, Default)]
#[repr(C)]
pub struct sigaltstack_t {
    pub ss_sp: Vaddr, // *mut c_void
    pub ss_flags: i32,
    pub ss_size: usize,
}

#[derive(Debug, Clone, Copy, Pod, Default)]
#[repr(C)]
pub struct mcontext_t {
    pub inner: SignalCpuContext,
    // TODO: the fields should be csgsfs, err, trapno, oldmask, and cr2
    _unused0: [u64; 5],
    // TODO: this field should be `fpregs: fpregset_t,`
    _unused1: usize,
    _reserved: [u64; 8],
}

#[derive(Debug, Clone, Copy, Pod, Default)]
#[repr(C)]
pub struct SignalCpuContext {
    pub gp_regs: GpRegs,
    pub fpregs_on_heap: u64,
    pub fpregs: Vaddr, // *mut FpRegs,
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct _sigev_thread {
    pub function: Vaddr,
    pub attribute: Vaddr,
}

const SIGEV_MAX_SIZE: usize = 64;
/// The total size of the fields `sigev_value`, `sigev_signo` and `sigev_notify`.
const SIGEV_PREAMBLE_SIZE: usize = size_of::<i32>() * 2 + size_of::<sigval_t>();
const SIGEV_PAD_SIZE: usize = (SIGEV_MAX_SIZE - SIGEV_PREAMBLE_SIZE) / size_of::<i32>();

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub union _sigev_un {
    pub _pad: [i32; SIGEV_PAD_SIZE],
    pub _tid: i32,
    pub _sigev_thread: _sigev_thread,
}

impl _sigev_un {
    pub fn read_tid(&self) -> i32 {
        read_union_fields!(self._tid)
    }

    pub fn read_function(&self) -> Vaddr {
        read_union_fields!(self._sigev_thread.function)
    }

    pub fn read_attribute(&self) -> Vaddr {
        read_union_fields!(self._sigev_thread.attribute)
    }
}

#[derive(Debug, Copy, Clone, TryFromInt, PartialEq)]
#[repr(i32)]
pub enum SigNotify {
    SIGEV_SIGNAL = 0,
    SIGEV_NONE = 1,
    SIGEV_THREAD = 2,
    SIGEV_THREAD_ID = 4,
}

#[derive(Clone, Copy, Pod)]
#[repr(C)]
pub struct sigevent_t {
    pub sigev_value: sigval_t,
    pub sigev_signo: i32,
    pub sigev_notify: i32,
    pub sigev_un: _sigev_un,
}
