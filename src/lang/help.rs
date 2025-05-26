/// A trait that descibes anything that the `help` command of Crush can operate on to provide
/// usage information for.

pub trait Help {
    fn signature(&self) -> String;
    fn short_help(&self) -> String;
    fn long_help(&self) -> Option<String>;
}
