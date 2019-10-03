use crate::commands::Namespace;

pub struct State {
    pub namespace: Namespace,
}

impl State {
  pub fn new() -> State {
      return State {
          namespace: Namespace::new(),
      }
  }
}
