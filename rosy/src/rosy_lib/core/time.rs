/// Returns CPU time consumed by this process in seconds, matching COSY INFINITY's CPUSEC.
///
/// Uses POSIX `getrusage(RUSAGE_SELF)` (user + system time) on Unix.
/// Falls back to 0.0 on non-Unix platforms (CPU time unavailable without platform APIs).
pub fn rosy_cpu_time() -> f64 {
    #[cfg(unix)]
    {
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        // SAFETY: `usage` is zero-initialized to a valid layout; RUSAGE_SELF is always valid.
        unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
        let user = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 / 1_000_000.0;
        let sys = usage.ru_stime.tv_sec as f64 + usage.ru_stime.tv_usec as f64 / 1_000_000.0;
        user + sys
    }
    #[cfg(not(unix))]
    {
        0.0_f64
    }
}
