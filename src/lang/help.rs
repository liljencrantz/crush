/// A trait that descibes anything that the `help` command of Crush can operate on to provide
/// usage information for.

pub trait Help {
    /// The signature for this item. For a command, this will be a listing of the arguments to the command. Supports markdown.
    fn signature(&self) -> String;
    /// A single-line description of the item. Supports markdown.
    fn short_help(&self) -> String;
    /// A multi-line description of the item. Supports markdown.
    fn long_help(&self) -> Option<String>;
}
