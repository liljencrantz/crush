use crate::commands::Namespace;

pub struct State {
    pub commands: Namespace,
}

impl State {
  pub fn new() -> State {
      return State {
          commands: Namespace::new(),
      }
  }
}
