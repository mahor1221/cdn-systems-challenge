// TODO: test new_random_set covers all world
// TODO: test direction_to
// TODO: test HOUSES_NEEDING_REPAIR > MAX_X * MAX_Y
// TODO: doc comment on every function for proper usage and example doc test
// TODO: explain why Array2 is used
// TODO: check for memory leak

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
use std::{
  collections::BTreeMap,
  fmt::{Display, Formatter, Result as FmtResult},
  io::stdout,
  thread,
  time::Duration,
};

const FRAME_DURATION_MS: u64 = 300;

fn main() {
  struct City1;
  impl WorldConfig for City1 {}
  // impl WorldConfig for City1 {
  //   const MAX_X: usize = 87;
  //   const MAX_Y: usize = 44;
  //   const REPAIRMANS: usize = 10;
  //   const HOUSES_NEEDING_REPAIR: usize = 3828;
  // }

  match World::<City1>::new().run() {
    Err(e) => eprintln!("{e}"),
    Ok(list) => println!("{list}"),
  }
}

#[derive(Debug, Default)]
struct List(BTreeMap<Id, Notes>);

impl<C: WorldConfig + Sync> World<C> {
  fn run(self: World<C>) -> CdnResult<List> {
    thread::scope(|s| {
      let mut handles = Vec::new();
      let world = &self;
      let barrier = Barrier::new();
      for id in world.get_repairmans_ids() {
        let bar = barrier.clone();
        let h = s.spawn(move || unsafe { Repairman::new(id, bar, world).work_loop() });
        handles.push(h);
      }

      let mut list = List::default();
      stdout().execute(Clear(ClearType::All))?;
      while handles.len() > 0 {
        stdout().execute(MoveTo(0, 0))?.execute(Print(&self))?;

        let (finished, rest): (Vec<_>, Vec<_>) = handles.into_iter().partition(|h| h.is_finished());
        handles = rest;
        for h in finished {
          let (id, notes) = h.join()??;
          list.0.insert(id, notes);
        }

        // The purpose of these two lines is to slow down the program for better
        // visualization of the result, they can be removed otherwise.
        barrier.wait();
        thread::sleep(Duration::from_millis(FRAME_DURATION_MS));
      }

      Ok(list)
    })
  }
}

impl Display for List {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    for (id, notes) in &self.0 {
      let t = notes.as_ref().iter().fold(0, |t, (_, n)| t + n);
      let r = notes.as_ref().get(&id).cloned().unwrap_or_default();
      let v: Vec<_> = notes.as_ref().iter().map(|n| *n.1).collect();
      writeln!(f, "{id:2?}, Repaired({r:2}). Notes({v:?}), Total({t})")?;
    }
    Ok(())
  }
}
