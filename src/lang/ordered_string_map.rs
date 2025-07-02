use ordered_map::OrderedMap;

/// The type `OrderedStringMap<T>`, which is an alias of `OrderedMap<String, T>.
/// It is used in signature parsing.
pub type OrderedStringMap<T> = OrderedMap<String, T>;
