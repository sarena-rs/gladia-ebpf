use aya_ebpf::bindings::tcx_action_base::{TCX_DROP, TCX_NEXT, TCX_PASS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Drop,
    Next,
    Redirect(i32),
}

impl From<Verdict> for i32 {
    fn from(v: Verdict) -> Self {
        match v {
            Verdict::Pass => TCX_PASS,
            Verdict::Drop => TCX_DROP,
            Verdict::Next => TCX_NEXT,
            Verdict::Redirect(code) => code,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EbpfError {
    #[error("Internal Error")]
    InternalError,

    #[error("Protocol not supported: {0}")]
    UnsupportedProtocol(u8),

    #[error("Map error: {0}")]
    MapError(i32),
}

impl EbpfError {
    pub const fn verdict(&self) -> Verdict {
        match self {
            EbpfError::InternalError | EbpfError::MapError(_) => Verdict::Drop,

            EbpfError::UnsupportedProtocol(_) => Verdict::Pass,
        }
    }
}

pub type Res<T> = Result<T, EbpfError>;

#[inline(always)]
pub fn dispatch(result: Res<Verdict>) -> i32 {
    match result {
        Ok(v) => v.into(),
        Err(e) => e.verdict().into(),
    }
}
