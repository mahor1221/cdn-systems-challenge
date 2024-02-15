use std::{
  any::Any,
  error::Error,
  fmt::{Debug, Display, Formatter, Result as FmtResult},
  io::Error as IoError,
  sync::PoisonError,
};

pub type CdnResult<T> = Result<T, CdnError>;

// Boxing [CdnErrorKind] reduces the size of [CdnResult] and enhances the
// overall performance of the program.
#[derive(Debug)]
pub struct CdnError(Box<CdnErrorKind>);

// This type is copied from the error part of `std::thread::Result`
type ThreadError = Box<dyn Any + Send + 'static>;

#[derive(Debug)]
pub enum CdnErrorKind {
  InvalidMoveDirection,
  PoisonError,
  IoError(IoError),
  ThreadError(ThreadError),
}

impl Error for CdnError {}
impl Display for CdnError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "{:?}", self.0)
  }
}

impl From<CdnErrorKind> for CdnError {
  fn from(value: CdnErrorKind) -> Self {
    CdnError(Box::new(value))
  }
}

impl<E> From<PoisonError<E>> for CdnError {
  fn from(_: PoisonError<E>) -> Self {
    CdnErrorKind::PoisonError.into()
  }
}

impl From<IoError> for CdnError {
  fn from(e: IoError) -> Self {
    CdnErrorKind::IoError(e).into()
  }
}

impl From<ThreadError> for CdnError {
  fn from(e: ThreadError) -> Self {
    CdnErrorKind::ThreadError(e).into()
  }
}
