// TODO: remove
#![allow(dead_code)]

use crossterm::{
  cursor::MoveTo,
  style::Print,
  terminal::{Clear, ClearType},
  ExecutableCommand,
};
use std::{io::stdout, thread, time::Duration};

use position::*;
use repairman::*;
use world::*;

mod position {
  use crate::WorldConfig;
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
  #[derive(Debug, Default, PartialEq, Eq, Hash)]
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

    pub fn r#move(&mut self, direction: MoveDirection) -> Option<()> {
      match direction {
        MoveDirection::Right if self.inner.x < C::MAX_X - 1 => {
          self.inner.x += 1;
          Some(())
        }
        MoveDirection::Left if self.inner.x > 0 => {
          self.inner.x -= 1;
          Some(())
        }
        MoveDirection::Up if self.inner.y < C::MAX_Y - 1 => {
          self.inner.y += 1;
          Some(())
        }
        MoveDirection::Down if self.inner.y > 0 => {
          self.inner.y -= 1;
          Some(())
        }
        _ => None,
      }
    }
  }

  //
  // boilerplate
  //

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
  use crate::{MoveDirection, Position};
  use ndarray::Array2;
  use owo_colors::{OwoColorize, Style};
  use rand::Rng;
  use std::{
    fmt::{Debug, Display, Error as FmtError, Formatter, Result as FmtResult, Write},
    sync::{Arc, Mutex, OnceLock},
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
    houses: Array2<Arc<Mutex<House>>>,
    repairmans: Vec<Arc<Mutex<Position<C>>>>,
  }

  impl<C: WorldConfig> World<C> {
    pub fn new() -> Self {
      if C::MAX_X * C::MAX_Y < C::HOUSES_NEEDING_REPAIR {
        panic!("MAX_X * MAX_Y must be bigger than HOUSES_NEEDING_REPAIR")
      }

      let rng = &mut rand::thread_rng();
      let repairmans = (0..C::REPAIRMANS)
        .map(|_| Arc::new(Mutex::new(rng.gen())))
        .collect();
      let houses: Array2<Arc<Mutex<House>>> = Array2::default((C::MAX_Y, C::MAX_X));

      for pos in Position::<C>::new_random_set(rng, C::HOUSES_NEEDING_REPAIR) {
        let mut house = houses[pos].lock().unwrap_or_else(|_| unreachable!());
        house.status = HouseStatus::NeedsRepair;
      }

      Self { houses, repairmans }
    }

    pub fn get_repairman_position(&self, repairman_id: usize) -> Arc<Mutex<Position<C>>> {
      Arc::clone(&self.repairmans[repairman_id])
    }

    pub fn get_repairman_house(&self, repairman_id: usize) -> Arc<Mutex<House>> {
      let pos = self.repairmans[repairman_id].lock().unwrap();
      Arc::clone(&self.houses[&*pos])
    }

    pub fn move_repairman(
      &self,
      repairman_id: usize,
      direction: MoveDirection,
    ) -> Option<Arc<Mutex<House>>> {
      let mut pos = self.repairmans[repairman_id].lock().ok()?;
      pos.r#move(direction);
      Some(Arc::clone(&self.houses[&*pos]))
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
  use crate::{House, HouseStatus, MoveDirection, Notes, Position, World, WorldConfig};
  use ndarray::Array2;
  use rand::random;
  use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
  };

  struct WorldMap<C: WorldConfig> {
    houses: Array2<HouseStatus>,
    phantom: PhantomData<C>,
  }

  impl<C: WorldConfig> Default for WorldMap<C> {
    fn default() -> Self {
      Self {
        houses: Array2::from_elem((C::MAX_Y, C::MAX_X), HouseStatus::NeedsRepair),
        phantom: PhantomData,
      }
    }
  }

  pub struct Repairman<'a, C: WorldConfig> {
    id: usize,
    map: WorldMap<C>,
    notes: Notes,
    position: Arc<Mutex<Position<C>>>,
    house: Arc<Mutex<House>>,
    fn_move: Box<dyn Fn(MoveDirection) -> Option<Arc<Mutex<House>>> + 'a + Send + Sync>,
  }

  impl<'a, C: WorldConfig + Sync + Send> Repairman<'a, C> {
    pub fn new(id: usize, world: &'a World<C>) -> Self {
      Self {
        id,
        position: world.get_repairman_position(id),
        house: world.get_repairman_house(id),
        fn_move: Box::new(move |d| world.move_repairman(id, d)),
        map: Default::default(),
        notes: Default::default(),
      }
    }

    pub fn work_loop(&mut self) -> Option<()> {
      loop {
        if self.house.lock().ok()?.status == HouseStatus::NeedsRepair {
          self.repair();
        }
        let dir: MoveDirection = random();
        self.r#move(dir).unwrap();

        thread::sleep(Duration::from_secs(1));
        // break Some(());
      }
    }

    fn idle(&self) {}

    fn r#move(&mut self, direction: MoveDirection) -> Option<()> {
      self.house = (&self.fn_move)(direction)?;
      Some(())
    }

    fn repair(&mut self) -> Option<()> {
      let pos = self.position.lock().ok()?;
      self.map.houses[&*pos] = HouseStatus::Repaired;
      self.house.lock().ok()?.status = HouseStatus::Repaired;
      Some(())
    }
  }
}

//

fn main() {
  // TODO: check REPAIRMANS < CPU COUNT
  // TODO: test new_random_set covers all world
  // TODO: test HOUSES_NEEDING_REPAIR > MAX_X * MAX_Y

  struct City1;
  impl WorldConfig for City1 {
    const MAX_X: usize = 14;
    const MAX_Y: usize = 14;
    const HOUSES_NEEDING_REPAIR: usize = 196;
  }

  let city1 = World::<City1>::new();

  let _ = thread::scope(|s| {
    let city = &city1;
    let handles: Vec<_> = (0..City1::REPAIRMANS)
      .map(|id| s.spawn(move || Repairman::new(id, city).work_loop()))
      .collect();

    loop {
      stdout()
        .execute(Clear(ClearType::All))
        .unwrap()
        .execute(MoveTo(0, 0))
        .unwrap()
        .execute(Print(&city1))
        .unwrap();
      thread::sleep(Duration::from_secs(1));
    }

    // handles.into_iter().fold((), |(), h| h.join().unwrap());
  });
}
