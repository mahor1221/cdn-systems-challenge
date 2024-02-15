use self::sync_cell::SyncCell;
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

static HOUSE_NEEDS_REPAIR_STYLE: OnceLock<OwoStyle> = OnceLock::new();
static HOUSE_REPAIRED_STYLE: OnceLock<OwoStyle> = OnceLock::new();

// `WorldConfig` is implemented as a trait to differentiate between `World`s and
// `Position`s of different sizes at compile time.
pub trait WorldConfig {
  const MAX_LEN_X: usize = 7;
  const MAX_LEN_Y: usize = 7;
  const REPAIRMEN: usize = 4;
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
  // The unsafe [`SyncCell`] is used to eliminate the need for using Mutexes,
  // as each `Repairman` will only change their own `Position`.
  repairmen: Vec<SyncCell<Position<C>>>,
}

impl<C: WorldConfig> Default for World<C> {
  fn default() -> Self {
    Self {
      repairmen: (0..C::REPAIRMEN).map(|_| Default::default()).collect(),
      houses: Array2::default((C::MAX_LEN_Y, C::MAX_LEN_X)),
    }
  }
}

impl<C: WorldConfig> World<C> {
  /// Creates a new world with houses requiring repair and repairmen scattered
  /// randomly across it.
  pub fn new() -> Self {
    if C::MAX_LEN_X * C::MAX_LEN_Y < C::HOUSES_NEEDING_REPAIR {
      panic!("MAX_X * MAX_Y must be bigger than HOUSES_NEEDING_REPAIR")
    }

    let rng = &mut rand::thread_rng();
    let houses: Array2<Mutex<House>> = Array2::default((C::MAX_LEN_Y, C::MAX_LEN_X));
    for pos in Position::<C>::new_random_set(rng, C::HOUSES_NEEDING_REPAIR) {
      let mut house = houses[pos].lock().unwrap_or_else(|_| unreachable!());
      house.status = HouseStatus::NeedsRepair;
    }

    let repairmen = (0..C::REPAIRMEN)
      .map(|_| SyncCell::new(rng.gen()))
      .collect();

    Self { houses, repairmen }
  }

  pub fn get_repairmen_ids(&self) -> impl Iterator<Item = Id> + '_ {
    self.repairmen.iter().enumerate().map(|(id, _)| id.into())
  }

  /// # Safety
  /// This is safe if [`Self::move_repairman`] is used correctly.
  pub unsafe fn get_repairman_position(&self, id: Id) -> &Position<C> {
    self.repairmen[id].get()
  }

  /// # Safety
  /// This is safe if [`Self::move_repairman`] is used correctly.
  pub unsafe fn get_repairman_house(&self, id: Id) -> &Mutex<House> {
    let pos = self.repairmen[id].get();
    &self.houses[pos]
  }

  /// # Safety
  /// Two threads must not pass the same [`Id`] to this method without
  /// synchronization.
  pub unsafe fn move_repairman(
    &self,
    id: Id,
    direction: MoveDirection,
  ) -> CdnResult<&Mutex<House>> {
    self.repairmen[id].get_mut().r#move(direction)?;
    Ok(&self.houses[self.repairmen[id].get()])
  }
}

impl<C: WorldConfig> Display for World<C> {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    for (y, row) in self.houses.outer_iter().enumerate() {
      for (x, house) in row.iter().enumerate() {
        let pos = Position::<C>::new(x, y);
        // This is safe if [`Self::move_repairman`] is used correctly.
        let i = unsafe { self.repairmen.iter().filter(|p| *p.get() == pos).count() };
        let repairmen_num = if i == 0 { "-".into() } else { i.to_string() };

        let s = match house.lock().map_err(|_| FmtError)?.status {
          HouseStatus::Repaired => C::house_repaired_style(),
          HouseStatus::NeedsRepair => C::house_needs_repair_style(),
        };
        write!(f, " {}", repairmen_num.style(*s))?;
      }
      f.write_char('\n')?;
    }

    Ok(())
  }
}

//
//  SyncCell
//

mod sync_cell {
  use std::cell::UnsafeCell;

  /// This is a simple wrapper around [`std::cell::UnsafeCell`]. In comparison
  /// with Mutex, it's zero-cost and adds no overhead to the program.
  #[derive(Debug)]
  pub struct SyncCell<T>(UnsafeCell<T>);
  unsafe impl<T: Sync> Sync for SyncCell<T> {}

  impl<T> SyncCell<T> {
    #[inline(always)]
    pub const fn new(value: T) -> Self {
      Self(UnsafeCell::new(value))
    }

    /// It's UB if the inner value gets deallocated while &T is alive.
    #[inline(always)]
    pub unsafe fn get(&self) -> &T {
      unsafe { &*self.0.get() }
    }

    /// It's UB if two threads write to the same value without synchronization.
    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn get_mut(&self) -> &mut T {
      &mut *self.0.get()
    }
  }

  impl<T: Default> Default for SyncCell<T> {
    fn default() -> Self {
      Self(UnsafeCell::new(T::default()))
    }
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

#[cfg(test)]
pub mod test {
  use std::sync::OnceLock;

  use super::{HouseStatus, World, WorldConfig};
  use crate::position::{MoveDirection, Position};
  use owo_colors::Style as OwoStyle;

  static HOUSE_NEEDS_REPAIR_STYLE: OnceLock<OwoStyle> = OnceLock::new();
  static HOUSE_REPAIRED_STYLE: OnceLock<OwoStyle> = OnceLock::new();
  pub struct Tst;
  impl WorldConfig for Tst {
    const MAX_LEN_X: usize = 4;
    const MAX_LEN_Y: usize = 3;
    const REPAIRMEN: usize = 3;
    const HOUSES_NEEDING_REPAIR: usize = 6;

    fn house_repaired_style<'a>() -> &'a OwoStyle {
      HOUSE_REPAIRED_STYLE.get_or_init(OwoStyle::new)
    }

    fn house_needs_repair_style<'a>() -> &'a OwoStyle {
      HOUSE_NEEDS_REPAIR_STYLE.get_or_init(|| OwoStyle::new().bold())
    }
  }

  #[test]
  #[should_panic]
  fn test_new_world() {
    struct WrongConfig;
    impl WorldConfig for WrongConfig {
      const MAX_LEN_X: usize = 2;
      const MAX_LEN_Y: usize = 2;
      const HOUSES_NEEDING_REPAIR: usize = 5;
    }
    World::<WrongConfig>::new();
  }

  #[test]
  fn test_move_repairman() {
    let pos1 = Position::new(0, 0);
    let pos2 = Position::new(1, 0);

    let world = World::<Tst>::default();
    for id in world.get_repairmen_ids() {
      let repairman_pos = unsafe { world.get_repairman_position(id) };
      assert_eq!(*repairman_pos, pos1);
      unsafe { world.move_repairman(id, MoveDirection::Right).unwrap() };
      assert_eq!(*repairman_pos, pos2);
    }
  }

  #[test]
  fn test_display_world() {
    let world = World::<Tst>::default();
    world.houses[[2, 3]].lock().unwrap().status = HouseStatus::NeedsRepair;
    unsafe { *world.repairmen[1].get_mut() = Position::new(2, 1) };

    let s = " 2 - - -\n - - 1 -\n - - - \u{1b}[1m-\u{1b}[0m\n";
    assert_eq!(s, &world.to_string());
  }
}
