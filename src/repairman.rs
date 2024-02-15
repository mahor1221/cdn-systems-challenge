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
  ops::{Index, IndexMut},
  sync::Mutex,
};

enum PathFindingResult {
  CurrentHouseIsUnexplored,
  NoUnexploredHouseFound,
  UnexploredHouseFound(MoveDirection),
}

/// An unique identifier for [`Repairman`].
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(usize);

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
enum MapStatus {
  #[default]
  Unexplored,
  Explored,
}

type FnMove<'a> = Box<dyn Fn(MoveDirection) -> CdnResult<&'a Mutex<House>> + 'a>;

///
pub struct Repairman<'a, C: WorldConfig> {
  id: Id,
  world_map: Array2<MapStatus>,
  notebook: Notes,
  position: &'a Position<C>,
  house: &'a Mutex<House>,
  barrier: Barrier,
  fn_move: FnMove<'a>,
}

impl<'a, C: WorldConfig + Sync> Repairman<'a, C> {
  /// Creates a new Repairman. [`Barrier`] is used for communication between
  /// repairmen. It's undefined behavior if two repairmen use the same `Id`.
  pub unsafe fn new(id: impl Into<Id>, barrier: Barrier, world: &'a World<C>) -> Self {
    let inner = |id| Self {
      id,
      barrier,
      world_map: Array2::default((C::MAX_LEN_Y, C::MAX_LEN_X)),
      notebook: Default::default(),
      position: world.get_repairman_position(id),
      house: world.get_repairman_house(id),
      // The move_repairman method is implemented as a closure to ensure that
      // each repairman can only modify their own position.
      // This is done to comply with the challenge rules.
      fn_move: Box::new(move |dir| unsafe { world.move_repairman(id, dir) }),
    };

    inner(id.into())
  }

  /// This is the primary decision-making function of the Repairman.
  /// It completes its work whenever one of these conditions is met:
  /// 1. There are no unexplored houses remaining on the map.
  /// 2. The total number of repaired houses inside the repairman's notebook
  /// equals the number of houses needing repair.
  pub fn work(mut self) -> CdnResult<(Id, Notes)> {
    while self.get_total_num_repaired() < C::HOUSES_NEEDING_REPAIR {
      // To prevent deadlock between multiple repairmen in the same house,
      // try_lock() is used instead of lock().
      let status = match self.house.try_lock() {
        Ok(house) => house.status,
        Err(_) => {
          self.idle();
          continue;
        }
      };

      match status {
        HouseStatus::NeedsRepair => self.repair_and_write_note()?,
        HouseStatus::Repaired => self.write_note()?,
      }

      self.read_notes()?;
      self.world_map[self.position] = MapStatus::Explored;

      use PathFindingResult::*;
      match self.find_path() {
        UnexploredHouseFound(dir) => self.r#move(dir)?,
        CurrentHouseIsUnexplored => unreachable!(),
        NoUnexploredHouseFound => break,
      }
    }

    Ok((self.id, self.notebook))
  }

  /// Summarizes the number of repaired houses inside the notebook.
  fn get_total_num_repaired(&self) -> usize {
    self.notebook.as_ref().iter().fold(0, |r, (_, i)| r + *i)
  }

  /// Writes the number of repaired houses onto the house.
  fn write_note(&self) -> CdnResult<()> {
    if let Some(num_repaired) = self.notebook.as_ref().get(&self.id) {
      let mut house = self.house.lock()?;
      house.notes.as_mut().insert(self.id, *num_repaired);
    }
    Ok(())
  }

  /// Reads the notes inside the house and updates the notebook if necessary.
  fn read_notes(&mut self) -> CdnResult<()> {
    let house = self.house.lock()?;
    for (id, num) in house.notes.as_ref() {
      let local_num = self.notebook.as_mut().entry(*id).or_default();
      if *local_num < *num {
        *local_num = *num;
      }
    }
    Ok(())
  }

  // /// This function locates the nearest unexplored house on the map using the BFS
  // algorithm and then returns the direction to that house. The search direction
  // is randomized.
  fn find_path(&self) -> PathFindingResult {
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

    let success = |pos: &Position<C>| self.world_map[pos] == MapStatus::Unexplored;

    use PathFindingResult::*;
    match bfs(self.position, successors, success) {
      Some(path) if path.len() < 2 => CurrentHouseIsUnexplored,
      Some(path) => UnexploredHouseFound(self.position.direction_to(&path[1])),
      None => NoUnexploredHouseFound,
    }
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

  fn repair_and_write_note(&mut self) -> CdnResult<()> {
    self.barrier.wait();

    let mut house = self.house.lock()?;
    match house.status {
      HouseStatus::NeedsRepair => {
        let num_repaired = self.notebook.as_mut().entry(self.id).or_default();
        *num_repaired += 1;
        *house.notes.as_mut().entry(self.id).or_default() = *num_repaired;
        house.status = HouseStatus::Repaired;
      }
      HouseStatus::Repaired => {
        drop(house);
        self.write_note()?;
      }
    }
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

#[cfg(test)]
mod test {
  use super::Repairman;
  use crate::{
    barrier::Barrier,
    world::{test::Tst, World},
  };

  #[test]
  fn test_wrote_note() {
    let world = World::<Tst>::default();
    let id = 0.into();
    let mut man = unsafe { Repairman::new(id, Barrier::new(), &world) };

    man.write_note().unwrap();
    let num = man.house.lock().unwrap().notes.as_ref().get(&id).cloned();
    assert!(num.is_none());

    const TEST_NUM: usize = 3;
    man.notebook.as_mut().insert(id, TEST_NUM);
    man.write_note().unwrap();
    let num = *man.house.lock().unwrap().notes.as_ref().get(&id).unwrap();
    assert_eq!(TEST_NUM, num);
  }

  #[test]
  fn test_read_notes() {
    let world = World::<Tst>::default();
    let mut man = unsafe { Repairman::new(0, Barrier::new(), &world) };

    // only the bigger values must remain
    let mut house = man.house.lock().unwrap();
    let other_id1 = 3.into();
    let other_id2 = 4.into();
    house.notes.as_mut().insert(other_id1, 7);
    house.notes.as_mut().insert(other_id2, 10);
    man.notebook.as_mut().insert(other_id1, 5);
    man.notebook.as_mut().insert(other_id2, 12);
    drop(house);

    man.read_notes().unwrap();
    let num1 = *man.notebook.as_ref().get(&other_id1).unwrap();
    let num2 = *man.notebook.as_ref().get(&other_id2).unwrap();
    assert_eq!(7, num1);
    assert_eq!(12, num2);
  }
}
