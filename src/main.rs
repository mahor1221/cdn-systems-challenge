// TODO: explain why Array2 is used

mod barrier;
pub mod error;
pub mod position;
pub mod repairman;
pub mod world;

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

fn main() {
  struct City1;
  impl WorldConfig for City1 {
    // const MAX_LEN_X: usize = 7;
    // const MAX_LEN_Y: usize = 7;
    // const REPAIRMEN: usize = 4;
    // const HOUSES_NEEDING_REPAIR: usize = 6;
  }

  const FRAME_DURATION_MS: u64 = 300;
  match World::<City1>::new().run(FRAME_DURATION_MS) {
    Err(e) => eprintln!("{e}"),
    Ok(list) => println!("{list}"),
  }
}

/// Stores the result of each finished thread. See [`World.run`].
#[derive(Debug, Default)]
pub struct List(BTreeMap<Id, Notes>);

impl<C: WorldConfig + Sync> World<C> {
  /// This function spawns new threads for each [`Repairman`] in the world
  /// to execute their tasks. It then periodically prints the world to the
  /// standard output with a specified interval in milliseconds defined by
  /// `frame_duration_ms`.
  fn run(self: World<C>, frame_duration_ms: u64) -> CdnResult<List> {
    thread::scope(|s| {
      let mut handles = Vec::new();
      let world = &self;
      let barrier = Barrier::new();
      for id in world.get_repairmen_ids() {
        let bar = barrier.clone();
        let h = s.spawn(move || unsafe { Repairman::new(id, bar, world).work() });
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

        // These lines slow down the program for better visualization. They
        // can be removed if not needed.
        barrier.wait();
        thread::sleep(Duration::from_millis(frame_duration_ms));
      }

      Ok(list)
    })
  }
}

impl Display for List {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    let mut total = 0;
    for (id, notes) in &self.0 {
      let r = notes.as_ref().get(&id).cloned().unwrap_or_default();
      let n: Vec<_> = notes.as_ref().iter().map(|n| *n.1).collect();
      let s = notes.as_ref().iter().fold(0, |s, (_, n)| s + n);
      writeln!(f, "{id:2?}, Repaired({r:2}), Notes({n:?}), NotesSum({s})")?;
      total += r;
    }
    writeln!(f, "TotalRepaired({total})")?;
    Ok(())
  }
}
