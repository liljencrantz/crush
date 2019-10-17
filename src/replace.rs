pub trait Replace<T> {
    fn replace(&mut self, idx: usize, el: T) -> T;
}

impl<T> Replace<T> for Vec<T> {
    fn replace(&mut self, idx: usize, el: T) -> T {
        self.push(el);
        self.swap_remove(idx)
    }
}
