# `lsi`

`lsi` is a leaking string interner. In return for leaking, you get ludicrous speed. The "l" stands
for either "leaking" or "ludicrous", depending on your mood.

# When to use it

Use it when you'll quickly hit saturation on the number of distinct strings you'll need to store: compilers, http stacks, etc.

# How it works

`Istr` is usize-sized, and points directly to string data on the heap, where the layout is `[len, ...data]`.
This means that `Istr` is `Copy` and single-instruction equality checks.

When constructing a `Istr`, we use a global `hashbrown::HashSet` to see if the string already exists. `hashbrown` uses `ahash`,
which [benchmarks](https://docs.rs/crate/ahash/latest) show as the fastest mainstream str hasher.