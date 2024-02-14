// TODO: remove
#![allow(dead_code)]

mod barrier;
mod error;
mod position;
mod repairman;
mod world;

use crate::{
  barrier::Barrier,
  error::CdnResult,
  repairman::{Id, Repairman},
  world::{Notes, World, WorldConfig},
};
use crossterm::{
  cursor::MoveTo,
  style::Print,
  terminal::{Clear, ClearType},
  ExecutableCommand,
};
use std::{collections::BTreeMap, io::stdout, thread, time::Duration};

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

fn main() {
  // TODO: test new_random_set covers all world
  // TODO: test direction_to
  // TODO: test HOUSES_NEEDING_REPAIR > MAX_X * MAX_Y
  // TODO: doc comment on every function for proper usage and example doc test
  // TODO: explain why Array2 is used
  // TODO: BTreeMap => HashMap

  struct City1;
  impl WorldConfig for City1 {
    const MAX_X: usize = 7;
    const MAX_Y: usize = 7;
    const REPAIRMANS: usize = 4;
    const HOUSES_NEEDING_REPAIR: usize = 6;
  }
  let city1 = World::<City1>::new();

  let result = thread::scope(|s| -> CdnResult<_> {
    let city = &city1;
    let barrier = Barrier::new();
    let mut handles = Vec::new();
    for id in 0..City1::REPAIRMANS {
      let bar = barrier.clone();
      let h = s.spawn(move || Repairman::new(id, bar, city).work_loop());
      handles.push(h)
    }

    let mut list: BTreeMap<Id, Notes> = BTreeMap::new();
    while handles.len() > 0 {
      stdout()
        .execute(Clear(ClearType::All))?
        .execute(MoveTo(0, 0))?
        .execute(Print(&city1))?;

      let (finished, rest): (Vec<_>, Vec<_>) = handles.into_iter().partition(|h| h.is_finished());
      handles = rest;
      for h in finished {
        let (id, notes) = h.join()??;
        list.insert(id, notes);
      }

      // The purpose of these two lines is to slow down the program for better
      // visualization of the result, they can be removed otherwise.
      barrier.wait();
      thread::sleep(Duration::from_millis(300));
    }

    Ok(list)
  });

  match result {
    Err(e) => eprintln!("{e}"),
    Ok(list) => {
      for (id, notes) in list {
        let t = notes.as_ref().iter().fold(0, |t, (_, n)| t + n);
        let r = notes.as_ref().get(&id).cloned().unwrap_or_default();
        let v: Vec<_> = notes.as_ref().iter().map(|n| *n.1).collect();
        println!("{id:2?}, Repaired({r:2}). Notes({v:?}), Total({t})");
      }
    }
  }
}
