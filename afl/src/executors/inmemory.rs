use crate::executors::{Executor, ExitKind, HasObservers};
use crate::inputs::{HasTargetBytes, Input};
use crate::observers::ObserversTuple;
use crate::tuples::Named;
use crate::AflError;

#[cfg(feature = "std")]
#[cfg(unix)]
pub mod unix_signals {

    extern crate libc;
    use self::libc::{c_int, c_void, sigaction, siginfo_t};
    // Unhandled signals: SIGALRM, SIGHUP, SIGINT, SIGKILL, SIGQUIT, SIGTERM
    use self::libc::{
        SA_NODEFER, SA_SIGINFO, SIGABRT, SIGBUS, SIGFPE, SIGILL, SIGPIPE, SIGSEGV, SIGUSR2,
    };
    use std::io::{stdout, Write}; // Write brings flush() into scope
    use std::{mem, ptr};

    use crate::executors::ExitKind;
    use crate::inputs::Input;

    static mut JUMP_BUF: [u8; 1024] = [0; 1024]; // overallocate jmp_buf
    static mut IS_HANDLING_EXCEPTIONS: bool = false;

    extern "C" {
        fn setjmp(env: *mut u8) -> i32;
        fn longjmp(env: *mut u8, val: i32) -> !;
    }

    #[inline(always)]
    pub fn start_handling_exceptions() -> Option<ExitKind> {
        unsafe {
            IS_HANDLING_EXCEPTIONS = true;
            num::FromPrimitive::from_i32(setjmp(JUMP_BUF.as_mut_ptr()))
        }
    }

    #[inline(always)]
    pub fn stop_handling_exceptions() {
        unsafe {
            IS_HANDLING_EXCEPTIONS = false;
        }
    }

    pub unsafe extern "C" fn libaflrs_executor_inmem_handle_crash<I>(
        _sig: c_int,
        info: siginfo_t,
        _void: c_void,
    ) where
        I: Input,
    {
        if !IS_HANDLING_EXCEPTIONS {
            println!(
                "We died accessing addr {}, but are not in client...",
                info.si_addr() as usize
            );
        }

        #[cfg(feature = "std")]
        println!("Child crashed!");
        #[cfg(feature = "std")]
        let _ = stdout().flush();

        longjmp(JUMP_BUF.as_mut_ptr(), ExitKind::Crash as i32);
    }

    pub unsafe extern "C" fn libaflrs_executor_inmem_handle_timeout<I>(
        _sig: c_int,
        _info: siginfo_t,
        _void: c_void,
    ) where
        I: Input,
    {
        dbg!("TIMEOUT/SIGUSR2 received");
        if !IS_HANDLING_EXCEPTIONS {
            dbg!("TIMEOUT or SIGUSR2 happened, but currently not fuzzing.");
            return;
        }

        // TODO: send LLMP.
        println!("Timeout in fuzz run.");
        let _ = stdout().flush();

        longjmp(JUMP_BUF.as_mut_ptr(), ExitKind::Timeout as i32);
    }

    // TODO clearly state that manager should be static (maybe put the 'static lifetime?)
    pub unsafe fn setup_crash_handlers<I>()
    where
        I: Input,
    {
        let mut sa: sigaction = mem::zeroed();
        libc::sigemptyset(&mut sa.sa_mask as *mut libc::sigset_t);
        sa.sa_flags = SA_NODEFER | SA_SIGINFO;
        sa.sa_sigaction = libaflrs_executor_inmem_handle_crash::<I> as usize;
        for (sig, msg) in &[
            (SIGSEGV, "segfault"),
            (SIGBUS, "sigbus"),
            (SIGABRT, "sigabrt"),
            (SIGILL, "illegal instruction"),
            (SIGFPE, "fp exception"),
            (SIGPIPE, "pipe"),
        ] {
            if sigaction(*sig, &mut sa as *mut sigaction, ptr::null_mut()) < 0 {
                panic!("Could not set up {} handler", &msg);
            }
        }

        sa.sa_sigaction = libaflrs_executor_inmem_handle_timeout::<I> as usize;
        if sigaction(SIGUSR2, &mut sa as *mut sigaction, ptr::null_mut()) < 0 {
            panic!("Could not set up sigusr2 handler for timeouts");
        }
    }
}

#[cfg(feature = "std")]
#[cfg(unix)]
use unix_signals as os_signals;
#[cfg(feature = "std")]
#[cfg(not(unix))]
compile_error!("InMemoryExecutor not yet supported on this OS");

/// The inmem executor harness
type HarnessFunction<I> = fn(&dyn Executor<I>, &[u8]) -> ExitKind;

/// The inmem executor simply calls a target function, then returns afterwards.
pub struct InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    harness: HarnessFunction<I>,
    observers: OT,
    name: &'static str,
}

impl<I, OT> Executor<I> for InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    #[inline]
    fn run_target(&mut self, input: &I) -> Result<ExitKind, AflError> {
        let bytes = input.target_bytes();
        let jump_code: Option<ExitKind> = os_signals::start_handling_exceptions();
        if let Some(exit_kind) = jump_code {
            os_signals::stop_handling_exceptions();
            Ok(exit_kind)
        } else {
            let ret = (self.harness)(self, bytes.as_slice());
            os_signals::stop_handling_exceptions();
            Ok(ret)
        }
    }
}

impl<I, OT> Named for InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    fn name(&self) -> &str {
        self.name
    }
}

impl<I, OT> HasObservers<OT> for InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    #[inline]
    fn observers(&self) -> &OT {
        &self.observers
    }

    #[inline]
    fn observers_mut(&mut self) -> &mut OT {
        &mut self.observers
    }
}

impl<I, OT> InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    pub fn new(name: &'static str, harness_fn: HarnessFunction<I>, observers: OT) -> Self {
        Self {
            harness: harness_fn,
            observers: observers,
            name: name,
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::executors::inmemory::InMemoryExecutor;
    use crate::executors::{Executor, ExitKind};
    use crate::inputs::{HasTargetBytes, Input, TargetBytes};
    use crate::tuples::tuple_list;

    use serde::{Deserialize, Serialize};

    #[derive(Clone, Serialize, Deserialize, Debug)]
    struct NopInput {}
    impl Input for NopInput {}
    impl HasTargetBytes for NopInput {
        fn target_bytes(&self) -> TargetBytes {
            TargetBytes::Owned(vec![0])
        }
    }

    #[cfg(feature = "std")]
    fn test_harness_fn_nop(_executor: &dyn Executor<NopInput>, buf: &[u8]) -> ExitKind {
        println!("Fake exec with buf of len {}", buf.len());
        ExitKind::Ok
    }

    #[cfg(not(feature = "std"))]
    fn test_harness_fn_nop(_executor: &dyn Executor<NopInput>, _buf: &[u8]) -> ExitKind {
        ExitKind::Ok
    }

    #[test]
    fn test_inmem_exec() {
        let mut in_mem_executor = InMemoryExecutor::new("main", test_harness_fn_nop, tuple_list!());
        let mut input = NopInput {};
        assert!(in_mem_executor.run_target(&mut input).is_ok());
    }
}
