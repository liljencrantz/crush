use std::ops::Deref;
use std::sync::Arc;

pub trait Identity {
    fn id(&self) -> u64;
}

impl<T> Identity for Arc<T> {
    fn id(&self) -> u64 {
        self.deref() as *const T as u64
    }
}

#[cfg(test)]
mod tests {
    use super::Identity;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn arc_identity() {
        for j in 0..10 {
            println!("{}", j);
            let mut d = HashMap::new();
            for _ in 0..10_000 {
                let arc: Arc<String> = Arc::from("hello".to_string().repeat(10));
                let id = arc.id();
                println!("{}", id);
                assert_eq!(id, arc.id());
                assert!(!d.contains_key(&id));
                d.insert(id, arc);
                assert!(d.contains_key(&id));
            }
            for (k, v) in d {
                assert_eq!(k, v.id())
            }
        }
    }
}
