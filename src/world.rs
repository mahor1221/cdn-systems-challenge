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

// The initial size of a new Vec is zero, so Notes doesn't allocate memory until
// a Repairman pushes into it. The overhead of a grid of Houses should be very
// minimal
#[derive(Default, Debug)]
pub struct House {
  pub notes: Notes,
  pub status: HouseStatus,
}

#[derive(Debug)]
pub struct World<C: WorldConfig> {
  houses: Array2<Mutex<House>>,
  repairmans: Vec<Mutex<Position<C>>>,
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

    Self { houses, repairmans }
  }

  pub fn get_repairman_position(&self, id: Id) -> &Mutex<Position<C>> {
    &self.repairmans[id]
  }

  pub fn get_repairman_house(&self, id: Id) -> &Mutex<House> {
    let pos = self.repairmans[id].lock().unwrap();
    &self.houses[&*pos]
  }

  pub fn move_repairman<'a>(
    &'a self,
    id: Id,
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
