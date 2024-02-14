use crate::{
  barrier::Barrier,
  error::CdnResult,
  position::{MoveDirection, Position},
  world::{House, HouseStatus, Notes, World, WorldConfig},
};
use ndarray::Array2;
use pathfinding::directed::bfs::bfs;
use rand::{seq::SliceRandom, thread_rng};
use std::{
  marker::PhantomData,
  ops::{Index, IndexMut},
  sync::Mutex,
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(usize);

pub struct Repairman<'a, C: WorldConfig> {
  id: Id,
  world_map: WorldMap<C>,
  notes: Notes,
  position: &'a Mutex<Position<C>>,
  house: &'a Mutex<House>,
  barrier: Barrier,
  fn_move: FnMove<'a>,
}

impl<'a, C: WorldConfig + Sync + Send> Repairman<'a, C> {
  pub fn new(id: impl Into<Id>, barrier: Barrier, world: &'a World<C>) -> Self {
    let inner = |id| Self {
      id,
      barrier,
      position: world.get_repairman_position(id),
      house: world.get_repairman_house(id),
      fn_move: Box::new(move |dir| world.move_repairman(id, dir)),
      world_map: Default::default(),
      notes: Default::default(),
    };

    inner(id.into())
  }

  pub fn work_loop(mut self) -> CdnResult<(Id, Notes)> {
    while self.num_repaired() < C::HOUSES_NEEDING_REPAIR {
      // TODO: test for deadlock
      if self.house.lock()?.status == HouseStatus::NeedsRepair {
        self.repair()?;
      }

      {
        self.update_notes()?;
        let pos = self.position.lock()?;
        self.world_map.houses[&*pos] = MapStatus::Explored;
      }

      match self.get_path_to_nearest_unexplored_house()? {
        None => self.idle(),
        Some(vec) => {
          let dir = self.position.lock()?.direction_to(&vec[1]);
          self.r#move(dir)?;
        }
      }
    }

    Ok((self.id, self.notes))
  }

  pub fn num_repaired(&self) -> usize {
    self.notes.as_ref().iter().fold(0, |r, (_, i)| r + *i)
  }

  fn get_path_to_nearest_unexplored_house(&self) -> CdnResult<Option<Vec<Position<C>>>> {
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

    let success = |pos: &Position<C>| self.world_map.houses[pos] == MapStatus::Unexplored;

    Ok(bfs(&*start, successors, success))
  }

  fn update_notes(&mut self) -> CdnResult<()> {
    for (id, num_repaired) in self.house.lock()?.notes.as_ref() {
      if *id != self.id {
        let num_repaired = *num_repaired;
        let local_num_repaired = self.notes.as_mut().entry(*id).or_default();
        if *local_num_repaired < num_repaired {
          *local_num_repaired = num_repaired;
        }
      }
    }

    Ok(())
  }

  //
  // actions
  //

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
    let num_repaired = self.notes.as_mut().entry(self.id).or_default();
    *num_repaired += 1;
    let mut house = self.house.lock()?;
    *house.notes.as_mut().entry(self.id).or_default() = *num_repaired;
    house.status = HouseStatus::Repaired;
    Ok(())
  }
}

//
// boilerplate
//

impl AsRef<usize> for Id {
  fn as_ref(&self) -> &usize {
    &self.0
  }
}

impl From<usize> for Id {
  fn from(value: usize) -> Self {
    Self(value)
  }
}

impl<T> Index<Id> for Vec<T> {
  type Output = T;

  fn index(&self, index: Id) -> &Self::Output {
    &self[*index.as_ref()]
  }
}

impl<T> IndexMut<Id> for Vec<T> {
  fn index_mut(&mut self, index: Id) -> &mut Self::Output {
    &mut self[*index.as_ref()]
  }
}
