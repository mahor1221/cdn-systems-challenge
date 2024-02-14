// TODO: remove
#![allow(dead_code)]

use crossterm::{
  cursor::MoveTo,
  style::Print,
  terminal::{Clear, ClearType},
  ExecutableCommand,
};
use std::{io::stdout, thread, time::Duration};

use error::*;
use position::*;
use repairman::*;
use world::*;

mod error {
  use std::{
    any::Any,
    error::Error,
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    io::Error as IoError,
    sync::PoisonError,
  };

  // from std::thread::Result
  pub type ThreadError = Box<dyn Any + Send + 'static>;

  pub type CdnResult<T> = Result<T, CdnError>;

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
}

mod position {
  use crate::{CdnError, CdnResult, WorldConfig};
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

  impl<C: WorldConfig> Eq for Position<C> {}
  impl<C: WorldConfig> PartialEq for Position<C> {
    fn eq(&self, other: &Self) -> bool {
      self.inner.eq(&other.inner)
    }
  }

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
}

// mod a {
//   use std::cell::UnsafeCell;

//   #[derive(Copy, Clone)]
//   pub struct UnsafeSlice<'a, T> {
//     slice: &'a [UnsafeCell<T>],
//   }
//   unsafe impl<'a, T: Send + Sync> Send for UnsafeSlice<'a, T> {}
//   unsafe impl<'a, T: Send + Sync> Sync for UnsafeSlice<'a, T> {}

//   impl<'a, T> UnsafeSlice<'a, T> {
//     pub fn new(slice: &'a mut [T]) -> Self {
//       let ptr = slice as *mut [T] as *const [UnsafeCell<T>];
//       Self {
//         slice: unsafe { &*ptr },
//       }
//     }

//     /// It's UB if two threads write to the same index without synchronization
//     pub unsafe fn write(&self, i: usize, value: T) {
//       let ptr = self.slice[i].get();
//       *ptr = value;
//     }
//   }
// }

mod world {
  use crate::{CdnResult, MoveDirection, Position};
  use ndarray::Array2;
  use owo_colors::{OwoColorize, Style};
  use rand::Rng;
  use std::{
    fmt::{Debug, Display, Error as FmtError, Formatter, Result as FmtResult, Write},
    sync::{Barrier, Mutex, OnceLock},
  };

  static HOUSE_NEEDS_REPAIR_STYLE: OnceLock<Style> = OnceLock::new();
  static HOUSE_REPAIRED_STYLE: OnceLock<Style> = OnceLock::new();
  pub trait WorldConfig {
    const MAX_X: usize = 7;
    const MAX_Y: usize = 7;
    const REPAIRMANS: usize = 4;
    const HOUSES_NEEDING_REPAIR: usize = 6;

    fn house_repaired_style<'a>() -> &'a Style {
      HOUSE_REPAIRED_STYLE.get_or_init(|| {
        Style::new()
          .fg_rgb::<250, 250, 250>()
          .bg_rgb::<50, 50, 100>()
      })
    }

    fn house_needs_repair_style<'a>() -> &'a Style {
      HOUSE_NEEDS_REPAIR_STYLE.get_or_init(|| {
        Style::new()
          .fg_rgb::<250, 250, 250>()
          .bg_rgb::<200, 100, 100>()
      })
    }
  }

  #[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
  pub enum HouseStatus {
    #[default]
    Repaired,
    NeedsRepair,
  }

  #[derive(Default, Debug, Clone)]
  pub struct Notes(Vec<usize>);

  #[derive(Default, Debug)]
  pub struct House {
    pub notes: Notes,
    pub status: HouseStatus,
  }

  #[derive(Debug)]
  pub struct World<C: WorldConfig> {
    houses: Array2<Mutex<House>>,
    repairmans: Vec<Mutex<Position<C>>>,
    pub barrier: Barrier,
  }

  impl<C: WorldConfig> World<C> {
    pub fn new() -> Self {
      if C::MAX_X * C::MAX_Y < C::HOUSES_NEEDING_REPAIR {
        panic!("MAX_X * MAX_Y must be bigger than HOUSES_NEEDING_REPAIR")
      }

      let rng = &mut rand::thread_rng();
      let repairmans = (0..C::REPAIRMANS).map(|_| Mutex::new(rng.gen())).collect();

      let houses: Array2<Mutex<House>> = Array2::default((C::MAX_Y, C::MAX_X));
      for pos in Position::<C>::new_random_set(rng, C::HOUSES_NEEDING_REPAIR) {
        let mut house = houses[pos].lock().unwrap_or_else(|_| unreachable!());
        house.status = HouseStatus::NeedsRepair;
      }

      // The "+ 1" allows the main thread to control the program's speed, it
      // can be removed otherwise.
      let barrier = Barrier::new(C::REPAIRMANS + 1);

      Self {
        houses,
        repairmans,
        barrier,
      }
    }

    pub fn get_repairman_position(&self, id: usize) -> &Mutex<Position<C>> {
      &self.repairmans[id]
    }

    pub fn get_repairman_house(&self, id: usize) -> &Mutex<House> {
      let pos = self.repairmans[id].lock().unwrap();
      &self.houses[&*pos]
    }

    pub fn move_repairman<'a>(
      &'a self,
      id: usize,
      direction: MoveDirection,
    ) -> CdnResult<&'a Mutex<House>> {
      let mut pos = self.repairmans[id].lock()?;
      pos.r#move(direction)?;
      Ok(&self.houses[&*pos])
    }
  }

  impl<C: WorldConfig> Display for World<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
      for (y, row) in self.houses.outer_iter().enumerate() {
        for (x, house) in row.iter().enumerate() {
          let pos = Position::<C>::new(x, y);
          let i = self
            .repairmans
            .iter()
            .filter(|p| *p.lock().unwrap() == pos)
            .count();
          let repairmans_num = if i == 0 { "-".into() } else { i.to_string() };

          let s = match house.lock().map_err(|_| FmtError)?.status {
            HouseStatus::Repaired => C::house_repaired_style(),
            HouseStatus::NeedsRepair => C::house_needs_repair_style(),
          };
          write!(f, " {}", repairmans_num.style(*s))?;
        }
        f.write_char('\n')?;
      }

      Ok(())
    }
  }
}

mod repairman {
  use crate::{CdnResult, House, HouseStatus, MoveDirection, Notes, Position, World, WorldConfig};
  use ndarray::Array2;
  use pathfinding::directed::bfs::bfs;
  use rand::{seq::SliceRandom, thread_rng};
  use std::{
    marker::PhantomData,
    sync::{Barrier, Mutex},
  };

  #[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
  enum MapStatus {
    #[default]
    Unexplored,
    Explored,
  }

  struct WorldMap<C: WorldConfig> {
    houses: Array2<MapStatus>,
    phantom: PhantomData<C>,
  }

  impl<C: WorldConfig> Default for WorldMap<C> {
    fn default() -> Self {
      Self {
        houses: Array2::default((C::MAX_Y, C::MAX_X)),
        phantom: PhantomData,
      }
    }
  }

  type FnMove<'a> = Box<dyn Fn(MoveDirection) -> CdnResult<&'a Mutex<House>> + 'a + Send + Sync>;

  pub struct Repairman<'a, C: WorldConfig> {
    id: usize,
    map: WorldMap<C>,
    notes: Notes,
    position: &'a Mutex<Position<C>>,
    house: &'a Mutex<House>,
    barrier: &'a Barrier,
    fn_move: FnMove<'a>,
  }

  impl<'a, C: WorldConfig + Sync + Send> Repairman<'a, C> {
    pub fn new(id: usize, world: &'a World<C>) -> Self {
      Self {
        id,
        position: world.get_repairman_position(id),
        house: world.get_repairman_house(id),
        barrier: &world.barrier,
        fn_move: Box::new(move |d| world.move_repairman(id, d)),
        map: Default::default(),
        notes: Default::default(),
      }
    }

    pub fn get_path_to_nearest_unexplored_house(&self) -> CdnResult<Option<Vec<Position<C>>>> {
      let start = self.position.lock()?;

      let successors = |pos: &Position<C>| {
        use MoveDirection::*;
        let mut vec = vec![Right, Left, Up, Down];
        vec.shuffle(&mut thread_rng());
        vec
          .into_iter()
          .filter_map(|d| {
            let mut p = pos.clone();
            p.r#move(d).ok()?;
            Some(p)
          })
          .collect::<Vec<_>>()
      };

      let success = |pos: &Position<C>| self.map.houses[pos] == MapStatus::Unexplored;

      Ok(bfs(&*start, successors, success))
    }

    pub fn work_loop(&mut self) -> CdnResult<()> {
      loop {
        if self.house.lock()?.status == HouseStatus::NeedsRepair {
          self.repair()?;
        }

        {
          let pos = &*self.position.lock()?;
          self.map.houses[pos] = MapStatus::Explored;
        }

        match self.get_path_to_nearest_unexplored_house()? {
          None => self.idle(),
          Some(vec) => {
            let dir = self.position.lock()?.direction_to(&vec[1]);
            self.r#move(dir)?;
          }
        }
      }
    }

    fn idle(&self) {
      self.barrier.wait();
    }

    fn r#move(&mut self, direction: MoveDirection) -> CdnResult<()> {
      self.barrier.wait();
      self.house = (&self.fn_move)(direction)?;
      Ok(())
    }

    fn repair(&mut self) -> CdnResult<()> {
      self.barrier.wait();
      self.house.lock()?.status = HouseStatus::Repaired;
      Ok(())
    }
  }
}

//

fn main() -> CdnResult<()> {
  // TODO: check REPAIRMANS < CPU COUNT
  // TODO: test new_random_set covers all world
  // TODO: test HOUSES_NEEDING_REPAIR > MAX_X * MAX_Y
  // TODO: doc comment on every function for proper usage and example doc test

  struct City1;
  impl WorldConfig for City1 {}
  let city1 = World::<City1>::new();

  thread::scope(|s| -> CdnResult<()> {
    let city = &city1;
    let mut handles: Vec<_> = (0..City1::REPAIRMANS)
      .map(|id| s.spawn(move || Repairman::new(id, city).work_loop()))
      .collect();

    let mut some_handle = handles.pop();
    while let Some(handle) = some_handle {
      stdout()
        .execute(Clear(ClearType::All))?
        .execute(MoveTo(0, 0))?
        .execute(Print(&city1))?;

      if handle.is_finished() {
        handle.join()??;
        some_handle = handles.pop();
      } else {
        some_handle = Some(handle)
      }

      // The purpose of these two lines is to slow down the program for better
      // visualization of the result, they can be removed otherwise.
      city1.barrier.wait();
      thread::sleep(Duration::from_millis(500));
    }

    Ok(())
  })
}
