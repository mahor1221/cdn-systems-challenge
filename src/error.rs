use std::{
  any::Any,
  error::Error,
  fmt::{Debug, Display, Formatter, Result as FmtResult},
  io::Error as IoError,
  sync::PoisonError,
};

pub type CdnResult<T> = Result<T, CdnError>;

// This type is copied from the error part of std::thread::Result
type ThreadError = Box<dyn Any + Send + 'static>;

#[derive(Debug)]
pub enum CdnError {
  InvalidMoveDirection,
  PoisonError,
  IoError(IoError),
  ThreadError(ThreadError),
}

impl Error for CdnError {}
impl Display for CdnError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "{self:?}")
  }
}

impl<E> From<PoisonError<E>> for CdnError {
  fn from(_: PoisonError<E>) -> Self {
    // PoisonError doesn't implement Send as it consits of a MutexGaurd
    CdnError::PoisonError
  }
}

impl From<IoError> for CdnError {
  fn from(e: IoError) -> Self {
    CdnError::IoError(e)
  }
}

impl From<Box<dyn Any + Send>> for CdnError {
  fn from(e: Box<dyn Any + Send>) -> Self {
    CdnError::ThreadError(e)
  }
}
