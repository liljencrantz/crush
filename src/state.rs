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

    pub fn get_cwd(&self) -> String {
        return std::env::current_dir().expect("AAAA").to_str().expect("fdsa").to_string();
    }
}
