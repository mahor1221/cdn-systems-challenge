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
  repairman::Repairman,
  world::{World, WorldConfig},
};
use crossterm::{
  cursor::MoveTo,
  style::Print,
  terminal::{Clear, ClearType},
  ExecutableCommand,
};
use std::{io::stdout, thread, time::Duration};

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
    const MAX_X: usize = 10;
    const MAX_Y: usize = 10;
    const REPAIRMANS: usize = 10;
    const HOUSES_NEEDING_REPAIR: usize = 100;
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

    let mut some_handle = handles.pop();
    let mut notes = Vec::new();
    while let Some(handle) = some_handle {
      stdout()
        .execute(Clear(ClearType::All))?
        .execute(MoveTo(0, 0))?
        .execute(Print(&city1))?;

      if handle.is_finished() {
        let n = handle.join()??;
        notes.push(n);
        some_handle = handles.pop();
      } else {
        some_handle = Some(handle)
      }

      // The purpose of these two lines is to slow down the program for better
      // visualization of the result, they can be removed otherwise.
      barrier.wait();
      thread::sleep(Duration::from_millis(100));
    }

    Ok(notes)
  });

  match result {
    Ok(notes) => notes.into_iter().for_each(|note| println!("{note:?}")),
    Err(e) => eprintln!("{e}"),
  }
}
