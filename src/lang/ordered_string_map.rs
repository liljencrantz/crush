/// Define the type `OrderedStringMap<T>`, which is an alias of `OrderedMap<String, T`.
use ordered_map::OrderedMap;

pub type OrderedStringMap<T> = OrderedMap<String, T>;
