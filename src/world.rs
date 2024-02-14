use crate::{
  error::CdnResult,
  position::{MoveDirection, Position},
  repairman::Id,
};
use ndarray::Array2;
use owo_colors::{OwoColorize, Style as OwoStyle};
use rand::Rng;
use std::{
  collections::BTreeMap,
  fmt::{Debug, Display, Error as FmtError, Formatter, Result as FmtResult, Write},
  sync::{Mutex, OnceLock},
};

pub use self::sync_cell::SyncCell;
mod sync_cell {
  use std::cell::UnsafeCell;

  #[derive(Debug)]
  pub struct SyncCell<T>(UnsafeCell<T>);
  unsafe impl<T: Send> Send for SyncCell<T> {}
  unsafe impl<T: Sync> Sync for SyncCell<T> {}

  impl<T> SyncCell<T> {
    pub const fn new(value: T) -> Self {
      Self(UnsafeCell::new(value))
    }

    pub fn get(&self) -> &T {
      unsafe { &*self.0.get() }
    }

    /// It's UB if two threads write to the same value without synchronization
    pub unsafe fn get_mut(&self) -> &mut T {
      &mut *self.0.get()
    }
  }
}

static HOUSE_NEEDS_REPAIR_STYLE: OnceLock<OwoStyle> = OnceLock::new();
static HOUSE_REPAIRED_STYLE: OnceLock<OwoStyle> = OnceLock::new();
pub trait WorldConfig {
  const MAX_X: usize = 7;
  const MAX_Y: usize = 7;
  const REPAIRMANS: usize = 4;
  const HOUSES_NEEDING_REPAIR: usize = 6;

  fn house_repaired_style<'a>() -> &'a OwoStyle {
    HOUSE_REPAIRED_STYLE.get_or_init(|| {
      OwoStyle::new()
        .fg_rgb::<250, 250, 250>()
        .bg_rgb::<50, 50, 100>()
    })
  }

  fn house_needs_repair_style<'a>() -> &'a OwoStyle {
    HOUSE_NEEDS_REPAIR_STYLE.get_or_init(|| {
      OwoStyle::new()
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
pub struct Notes(BTreeMap<Id, usize>);

#[derive(Default, Debug)]
pub struct House {
  pub notes: Notes,
  pub status: HouseStatus,
}

#[derive(Debug)]
pub struct World<C: WorldConfig> {
  houses: Array2<Mutex<House>>,
  repairmans: Vec<SyncCell<Position<C>>>,
}

impl<C: WorldConfig> World<C> {
  pub fn new() -> Self {
    if C::MAX_X * C::MAX_Y < C::HOUSES_NEEDING_REPAIR {
      panic!("MAX_X * MAX_Y must be bigger than HOUSES_NEEDING_REPAIR")
    }

    let rng = &mut rand::thread_rng();
    let repairmans = (0..C::REPAIRMANS)
      .map(|_| SyncCell::new(rng.gen()))
      .collect();

    let houses: Array2<Mutex<House>> = Array2::default((C::MAX_Y, C::MAX_X));
    for pos in Position::<C>::new_random_set(rng, C::HOUSES_NEEDING_REPAIR) {
      let mut house = houses[pos].lock().unwrap_or_else(|_| unreachable!());
      house.status = HouseStatus::NeedsRepair;
    }

    Self { houses, repairmans }
  }

  pub fn get_repairman_position(&self, id: Id) -> &SyncCell<Position<C>> {
    &self.repairmans[id]
  }

  pub fn get_repairman_house(&self, id: Id) -> &Mutex<House> {
    let pos = self.repairmans[id].get();
    &self.houses[pos]
  }

  pub fn move_repairman<'a>(
    &'a self,
    id: Id,
    direction: MoveDirection,
  ) -> CdnResult<&'a Mutex<House>> {
    unsafe { self.repairmans[id].get_mut().r#move(direction)? };
    Ok(&self.houses[self.repairmans[id].get()])
  }
}

impl<C: WorldConfig> Display for World<C> {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    for (y, row) in self.houses.outer_iter().enumerate() {
      for (x, house) in row.iter().enumerate() {
        let pos = Position::<C>::new(x, y);
        let i = self.repairmans.iter().filter(|p| *p.get() == pos).count();
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

//
// boilerplate
//

impl AsRef<BTreeMap<Id, usize>> for Notes {
  fn as_ref(&self) -> &BTreeMap<Id, usize> {
    &self.0
  }
}

impl AsMut<BTreeMap<Id, usize>> for Notes {
  fn as_mut(&mut self) -> &mut BTreeMap<Id, usize> {
    &mut self.0
  }
}
