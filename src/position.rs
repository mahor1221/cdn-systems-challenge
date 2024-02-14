use crate::{
  error::{CdnError, CdnResult},
  world::WorldConfig,
};
use ndarray::{Dim, NdIndex};
use rand::{
  distributions::{Distribution, Standard},
  rngs::ThreadRng,
  seq::SliceRandom,
  Rng,
};
use std::{fmt::Debug, hash::Hash, marker::PhantomData};

#[derive(Clone, Copy, Debug)]
pub enum MoveDirection {
  Right,
  Left,
  Up,
  Down,
}

// To be able to derive traits without adding unnecessary constraints to
// the "C: WorldConfig" generic parameter, the non-generic part of Position
// is separated into PositionInner.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct PositionInner {
  x: usize,
  y: usize,
}

pub struct Position<C: WorldConfig> {
  inner: PositionInner,
  phantom: PhantomData<C>,
}

impl<C: WorldConfig> Position<C> {
  pub fn new(x: usize, y: usize) -> Self {
    Self {
      inner: PositionInner { x, y },
      phantom: PhantomData,
    }
  }

  pub fn new_random_set(rng: &mut ThreadRng, len: usize) -> Vec<Self> {
    let mut numbers: Vec<usize> = (0..C::MAX_X * C::MAX_Y).collect();
    numbers.shuffle(rng);
    numbers.truncate(len);
    numbers
      .into_iter()
      .map(|n| Self::new(n % C::MAX_X, n / C::MAX_X))
      .collect()
  }

  pub fn to_index(&self) -> [usize; 2] {
    [self.inner.y, self.inner.x]
  }

  pub fn r#move(&mut self, direction: MoveDirection) -> CdnResult<()> {
    match direction {
      MoveDirection::Right if self.inner.x < C::MAX_X - 1 => {
        self.inner.x += 1;
      }
      MoveDirection::Left if self.inner.x > 0 => {
        self.inner.x -= 1;
      }
      MoveDirection::Up if self.inner.y < C::MAX_Y - 1 => {
        self.inner.y += 1;
      }
      MoveDirection::Down if self.inner.y > 0 => {
        self.inner.y -= 1;
      }
      _ => return Err(CdnError::InvalidMoveDirection),
    }
    Ok(())
  }

  pub fn direction_to(&self, other: &Self) -> MoveDirection {
    if self.inner.x.checked_add(1) == Some(other.inner.x) {
      MoveDirection::Right
    } else if self.inner.x.checked_sub(1) == Some(other.inner.x) {
      MoveDirection::Left
    } else if self.inner.y.checked_add(1) == Some(other.inner.y) {
      MoveDirection::Up
    } else if self.inner.y.checked_sub(1) == Some(other.inner.y) {
      MoveDirection::Down
    } else {
      panic!("self and other are not adjacent positions")
    }
  }
}

impl Distribution<MoveDirection> for Standard {
  fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> MoveDirection {
    match rng.gen_range(0..=3) {
      0 => MoveDirection::Right,
      1 => MoveDirection::Left,
      2 => MoveDirection::Up,
      _ => MoveDirection::Down,
    }
  }
}

impl<C: WorldConfig> Distribution<Position<C>> for Standard {
  fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Position<C> {
    Position {
      inner: PositionInner {
        x: rng.gen_range(0..C::MAX_X),
        y: rng.gen_range(0..C::MAX_Y),
      },
      phantom: PhantomData,
    }
  }
}

//
// boilerplate
//

impl<C: WorldConfig> Clone for Position<C> {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
      phantom: PhantomData,
    }
  }
}

impl<C: WorldConfig> Debug for Position<C> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.inner.fmt(f)
  }
}

impl<C: WorldConfig> Default for Position<C> {
  fn default() -> Self {
    Self {
      inner: Default::default(),
      phantom: Default::default(),
    }
  }
}

impl<C: WorldConfig> PartialEq for Position<C> {
  fn eq(&self, other: &Self) -> bool {
    self.inner.eq(&other.inner)
  }
}

impl<C: WorldConfig> Eq for Position<C> {}

impl<C: WorldConfig> Hash for Position<C> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.inner.hash(state)
  }
}

unsafe impl<C: WorldConfig> NdIndex<Dim<[usize; 2]>> for Position<C> {
  fn index_checked(&self, dim: &Dim<[usize; 2]>, strides: &Dim<[usize; 2]>) -> Option<isize> {
    self.to_index().index_checked(dim, strides)
  }

  fn index_unchecked(&self, strides: &Dim<[usize; 2]>) -> isize {
    self.to_index().index_unchecked(strides)
  }
}

unsafe impl<C: WorldConfig> NdIndex<Dim<[usize; 2]>> for &Position<C> {
  fn index_checked(&self, dim: &Dim<[usize; 2]>, strides: &Dim<[usize; 2]>) -> Option<isize> {
    self.to_index().index_checked(dim, strides)
  }

  fn index_unchecked(&self, strides: &Dim<[usize; 2]>) -> isize {
    self.to_index().index_unchecked(strides)
  }
}
