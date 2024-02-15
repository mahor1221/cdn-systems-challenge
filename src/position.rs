use crate::{
  error::{CdnError, CdnResult},
  world::WorldConfig,
};
use core::panic;
use ndarray::{Dim, NdIndex};
use rand::{
  distributions::{Distribution, Standard},
  rngs::ThreadRng,
  seq::SliceRandom,
  Rng,
};
use std::{fmt::Debug, hash::Hash, marker::PhantomData};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    if x >= C::MAX_LEN_X || y >= C::MAX_LEN_Y {
      panic!("x and y must be smaller than MAX_X and MAX_Y")
    }

    Self {
      inner: PositionInner { x, y },
      phantom: PhantomData,
    }
  }

  pub fn new_random_set(rng: &mut ThreadRng, len: usize) -> Vec<Self> {
    let mut numbers: Vec<usize> = (0..C::MAX_LEN_X * C::MAX_LEN_Y).collect();
    numbers.shuffle(rng);
    numbers.truncate(len);
    numbers
      .into_iter()
      .map(|n| Self::new(n % C::MAX_LEN_X, n / C::MAX_LEN_X))
      .collect()
  }

  pub fn to_index(&self) -> [usize; 2] {
    [self.inner.y, self.inner.x]
  }

  pub fn r#move(&mut self, direction: MoveDirection) -> CdnResult<()> {
    match direction {
      MoveDirection::Right if self.inner.x < C::MAX_LEN_X - 1 => {
        self.inner.x += 1;
      }
      MoveDirection::Left if self.inner.x > 0 => {
        self.inner.x -= 1;
      }
      MoveDirection::Up if self.inner.y < C::MAX_LEN_Y - 1 => {
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
    if self.inner.x.checked_add(1) == Some(other.inner.x) && self.inner.y == other.inner.y {
      MoveDirection::Right
    } else if self.inner.x.checked_sub(1) == Some(other.inner.x) && self.inner.y == other.inner.y {
      MoveDirection::Left
    } else if self.inner.y.checked_add(1) == Some(other.inner.y) && self.inner.x == other.inner.x {
      MoveDirection::Up
    } else if self.inner.y.checked_sub(1) == Some(other.inner.y) && self.inner.x == other.inner.x {
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
        x: rng.gen_range(0..C::MAX_LEN_X),
        y: rng.gen_range(0..C::MAX_LEN_Y),
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

#[cfg(test)]
mod test {
  use super::{MoveDirection::*, Position};
  use crate::world::{test::Tst, WorldConfig};
  use ndarray::{array, Array2};
  use rand::thread_rng;
  use std::collections::HashSet;

  const LEN: usize = Tst::MAX_LEN_X * Tst::MAX_LEN_Y;

  #[test]
  fn test_uniqueness_of_random_set() {
    let mut rng = thread_rng();
    let set: HashSet<_> = Position::<Tst>::new_random_set(&mut rng, LEN)
      .into_iter()
      .collect();
    assert_eq!(LEN, set.len())
  }

  #[test]
  fn test_position_to_index() {
    let arr2: Array2<usize> = array![[0, 1, 2, 3], [4, 5, 6, 7], [8, 9, 10, 11]];

    let idx = Position::<Tst>::new(2, 1).to_index();
    assert_eq!(6, arr2[idx]);

    let idx = Position::<Tst>::new(1, 2).to_index();
    assert_eq!(9, arr2[idx]);

    let idx = Position::<Tst>::new(3, 2).to_index();
    assert_eq!(11, arr2[idx]);
  }

  #[test]
  #[should_panic]
  fn test_new_position_1() {
    Position::<Tst>::new(0, 4);
  }

  #[test]
  #[should_panic]
  fn test_new_position_2() {
    Position::<Tst>::new(4, 0);
  }

  #[test]
  fn test_move_position() {
    let mut pos = Position::<Tst>::new(Tst::MAX_LEN_X - 1, Tst::MAX_LEN_Y - 1);
    pos.r#move(Up).unwrap_err();
    pos.r#move(Right).unwrap_err();

    let mut pos = Position::<Tst>::new(0, 0);
    pos.r#move(Left).unwrap_err();
    pos.r#move(Down).unwrap_err();

    pos.r#move(Right).unwrap();
    assert_eq!([0, 1], pos.to_index());
    pos.r#move(Left).unwrap();
    assert_eq!([0, 0], pos.to_index());
    pos.r#move(Up).unwrap();
    assert_eq!([1, 0], pos.to_index());
    pos.r#move(Down).unwrap();
    assert_eq!([0, 0], pos.to_index());
  }

  #[test]
  fn test_direction_to_position() {
    let pos1 = Position::<Tst>::new(1, 1);

    let pos2 = Position::<Tst>::new(2, 1);
    assert_eq!(Right, pos1.direction_to(&pos2));
    let pos2 = Position::<Tst>::new(1, 2);
    assert_eq!(Up, pos1.direction_to(&pos2));
    let pos2 = Position::<Tst>::new(0, 1);
    assert_eq!(Left, pos1.direction_to(&pos2));
    let pos2 = Position::<Tst>::new(1, 0);
    assert_eq!(Down, pos1.direction_to(&pos2));
  }

  #[test]
  #[should_panic]
  fn test_direction_to_non_adjacent_position_1() {
    let pos = Position::<Tst>::new(0, 0);
    pos.direction_to(&pos);
  }

  #[test]
  #[should_panic]
  fn test_direction_to_non_adjacent_position_2() {
    let pos1 = Position::<Tst>::new(0, 0);
    let pos2 = Position::<Tst>::new(1, 1);
    pos1.direction_to(&pos2);
  }
}
