

pub trait Help {
    fn signature(&self) -> String;
    fn short_help(&self) -> String;
    fn long_help(&self) -> Option<String>;
}
