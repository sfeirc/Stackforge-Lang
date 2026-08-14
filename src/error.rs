use std::fmt;

/// Every error the pipeline can produce, tagged with the phase it came from
/// and (whenever known) the 1-based source line, so the CLI can print
/// `error: <phase>: <message> (line N)` uniformly.
#[derive(Debug, Clone, PartialEq)]
pub enum SfError {
    Lex { message: String, line: u32 },
    Parse { message: String, line: u32 },
    Compile { message: String, line: u32 },
    Runtime { message: String, line: u32 },
}

impl SfError {
    pub fn lex(message: impl Into<String>, line: u32) -> Self {
        SfError::Lex {
            message: message.into(),
            line,
        }
    }
    pub fn parse(message: impl Into<String>, line: u32) -> Self {
        SfError::Parse {
            message: message.into(),
            line,
        }
    }
    pub fn compile(message: impl Into<String>, line: u32) -> Self {
        SfError::Compile {
            message: message.into(),
            line,
        }
    }
    pub fn runtime(message: impl Into<String>, line: u32) -> Self {
        SfError::Runtime {
            message: message.into(),
            line,
        }
    }

    pub fn line(&self) -> u32 {
        match self {
            SfError::Lex { line, .. }
            | SfError::Parse { line, .. }
            | SfError::Compile { line, .. }
            | SfError::Runtime { line, .. } => *line,
        }
    }

    pub fn phase(&self) -> &'static str {
        match self {
            SfError::Lex { .. } => "lex error",
            SfError::Parse { .. } => "syntax error",
            SfError::Compile { .. } => "compile error",
            SfError::Runtime { .. } => "runtime error",
        }
    }
}

impl fmt::Display for SfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            SfError::Lex { message, .. } => message,
            SfError::Parse { message, .. } => message,
            SfError::Compile { message, .. } => message,
            SfError::Runtime { message, .. } => message,
        };
        if self.line() > 0 {
            write!(f, "{} (line {}): {}", self.phase(), self.line(), msg)
        } else {
            write!(f, "{}: {}", self.phase(), msg)
        }
    }
}

impl std::error::Error for SfError {}

pub type SfResult<T> = Result<T, SfError>;
