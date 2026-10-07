/// Sets the status of the test to a [`TestStatus`](crate::TestStatus), without ending it.
///
/// ```rust,ignore
/// status!(t, TestStatus::Pass);
/// ```
#[macro_export]
macro_rules! status {
    ($t:ident, $status:expr) => {{
        $t.set_status($status);
    }};
}

/// Compares `len` bytes of the packet, starting at `offset`, with the expected buffer `buf`.
///
/// On a mismatch, or when the packet or `buf` is shorter than `len`, the test fails and both
/// buffers are sent to userspace. `name` identifies the assertion in the report, and
/// `first_layer` names the protocol layer the buffers start with (for example `"Ether"`).
/// Unlike [`assert_test!`](crate::assert_test), a failure does not end the test. At most
/// [`SCAPY_MAX_BUF`](crate::SCAPY_MAX_BUF) bytes are compared.
///
/// Only for `tc` programs.
///
/// ```rust,ignore
/// assert_buffer!(t, &ctx, "arp reply", "Ether", 0, &EXPECTED, EXPECTED.len());
/// ```
#[macro_export]
macro_rules! assert_buffer {
    ($t:ident, $ctx:expr, $name:literal, $first_layer:literal, $offset:expr, $buf:expr, $len:expr) => {{
        $t.assert_buffer_inner(
            file!(),
            line!(),
            $ctx,
            $name,
            $first_layer,
            $offset,
            $buf,
            $len,
            concat!("Buffer '", stringify!($buf), "' of len (%d) < LEN (%d)"),
            concat!("CTX and buffer '", stringify!($buf), "' content mismatch"),
        )
    }};
}

/// Logs a message, formatted in userspace.
///
/// The format string is a literal with `bpf_trace_printk` style specifiers (`%d`, `%u`, `%x`,
/// `%llu`, ...); the arguments are integers, converted to `u64`.
///
/// ```rust,ignore
/// test_log!(t, "checked %d of %d entries", done, total);
/// ```
#[macro_export]
macro_rules! test_log {
    // Zero args
    ($t:ident, $fmt:literal) => {{
        $t.log0(line!(), $fmt.as_bytes());
    }};
    // One arg
    ($t:ident, $fmt:literal, $a0:expr) => {{
        $t.log1(line!(), $fmt.as_bytes(), $a0 as u64);
    }};
    // Two args
    ($t:ident, $fmt:literal, $a0:expr, $a1:expr) => {{
        $t.log2(line!(), $fmt.as_bytes(), $a0 as u64, $a1 as u64);
    }};
    // Three args
    ($t:ident, $fmt:literal, $a0:expr, $a1:expr, $a2:expr) => {{
        $t.log3(line!(), $fmt.as_bytes(), $a0 as u64, $a1 as u64, $a2 as u64);
    }};
    // Four or more args — falls back to generic log with a slice
    ($t:ident, $fmt:literal, $($arg:expr),+ $(,)?) => {{
        $t.log(line!(), $fmt.as_bytes(), &[$($arg as u64),+]);
    }};
}

/// Logs a message, fails the test and ends it.
///
/// Ends the test by breaking out of the loop that [`macro@crate::assert`] wraps around the
/// function body, so it can only be used directly in the body of an `#[assert]` function, not in
/// a nested loop or another function.
///
/// ```rust,ignore
/// test_fatal!(t, "unexpected return code %d", ret);
/// ```
#[macro_export]
macro_rules! test_fatal {
    ($t:ident, $fmt:expr $(, $arg:expr)* $(,)?) => {{
        $t.log(line!(), $fmt.as_bytes(), &[$($arg as u64),*]);
        $t.fail();
        break;
    }};
}

/// Skips the test and ends it.
///
/// Like [`test_fatal!`](crate::test_fatal), it can only be used directly in the body of an
/// `#[assert]` function.
///
/// ```rust,ignore
/// if !feature_enabled() {
///     test_skip!(t);
/// }
/// ```
#[macro_export]
macro_rules! test_skip {
    ($t:ident) => {{
        $t.skip();
        break;
    }};
}

/// Fails and ends the test if a condition is false.
///
/// Without a message, the failed condition itself is logged. Like [`test_fatal!`], it can only be
/// used directly in the body of an `#[assert]` function.
///
/// ```rust,ignore
/// assert_test!(t, len >= 42);
/// assert_test!(t, len >= 42, "reply too short: %d bytes", len);
/// ```
#[macro_export]
macro_rules! assert_test {
    ($t:ident, $cond:expr) => {{
        if !($cond) {
            $crate::test_fatal!($t, concat!("assert failed: ", stringify!($cond)));
        }
    }};
    ($t:ident, $cond:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {{
        if !($cond) {
            $crate::test_fatal!($t, $fmt $(, $arg)*);
        }
    }};
}
