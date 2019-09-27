use crate::commands::CommandMap;

pub struct State {
    pub commands: crate::commands::CommandMap,
}

impl State {
  pub fn new() -> State {
      return State {
          commands: CommandMap::new(),
      }
  }
}
