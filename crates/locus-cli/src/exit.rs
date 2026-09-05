//! Stable process exit-code mapping from specification section 9.2.

/// Semantic CLI outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Exit {
    Success,
    CheckFailure,
    Evaluation,
    Usage,
}

impl Exit {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::CheckFailure => 1,
            Self::Evaluation => 2,
            Self::Usage => 3,
        }
    }
}

impl From<Exit> for std::process::ExitCode {
    fn from(value: Exit) -> Self {
        Self::from(value.code())
    }
}
