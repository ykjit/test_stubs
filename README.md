# test_stubs

Generate `todo!()` stubs in testing mode for trait methods without default
implementations.

## Overview

This library provides a proc macro attribute `test_stubs` which can be attached
to traits: for each method in the trait without a default implementation, two
variants will be created, one for `#[cfg(not(test))]` and one for
`#[cfg(test)]`. The latter will have a stubbed method body containing just
`todo!()`, allowing tests to implement the trait without having to manually
implement each method.

Roughly speaking, given the following Rust source file:

```text
#[test_stubs]
trait T {
  fn f(&self) { ... }
  fn g(&self);
}
```

will produce:

```text
trait T {
  fn f(&self) { ... }

  #[cfg(not(test))]
  fn g(&self);

  #[cfg(test)]
  fn g(&self) { todo!() }
}
```

Note: `f` was copied over unchanged, but two copies of `g` were generated, one
with and one without a default implementation.


## Limitations

There are limitation to what `test_stubs` can do.

For example, Rust's type inference isn't always happy with just `todo!()`. This
code will not compile:

```text
trait T {
  #[cfg(test)]
  fn f() -> impl Iterator<...> { todo!() }
}
```

There is no generic solution to this. `test_stubs` knows about some common
types and will generate code for them. For a type such as the above it will
generate:

```text
trait T {
  #[cfg(test)]
  fn f() -> impl Iterator<...> { todo!() as std::iter::Empty<_> }
}
```

When `test_stubs` has no specific knowledge about a type, it will simply
generate `todo!()` and hope.

If a trait method takes `self` (rather than `&self`), `test_stubs` will add a
`where Self: Sized` constraint to the `#[cfg(test)]` method.
