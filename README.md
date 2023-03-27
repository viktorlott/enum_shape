<div align="center">
    <img alt="Github" src="https://raw.githubusercontent.com/viktorlott/penum/main/penum-logo.png" height="103">
</div>

<div align="center">
    <a href="https://github.com/viktorlott/penum">
        <img alt="Github" src="https://img.shields.io/github/languages/code-size/viktorlott/penum?style=flat-square&logo=github" height="20">
        <img alt="Download" src="https://img.shields.io/crates/d/penum.svg?style=flat-square&logo=rust" height="20">
        <img alt="Tests" src="https://img.shields.io/github/actions/workflow/status/viktorlott/penum/test.yaml?branch=main&style=flat-square&logo=github">
    </a>
    <a href="https://crates.io/crates/penum">
        <img alt="crates.io" src="https://img.shields.io/crates/v/penum.svg?style=flat-square&logo=rust" height="20">
    </a>
    <a href="https://docs.rs/penum/latest/penum/">
        <img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-penum-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">
    </a>
</div>

`penum` is a procedural macro that is used for **enum conformity** and
**automatic dispatch**. This is done by specifying a declarative pattern
that expresses how we should interpret the enum. It's a tool for
asserting how enums should **look** and **behave** through simple
expressive rust grammar.

- **Patterns** — can be thought of as a *toy shape sorter* that sorts
  through enum variants and makes sure they fit. So each variant has a
  certain shape that must satisfy the patterns we've specified. There
  are 3 shapes to choose from, *tuples* `()`, *structs* `{}` and
  *units*.

- **Predicates** — are used in combination with *patterns* to assert
  what the matched variants field types should implement. They can be
  expressed like a regular where clause, e.g `where T: Trait<Type>`. The
  *generic parameters* needs to be introduced inside a pattern
  fragment.

- **Smart dispatch** — lets us express how an enum should **behave** in
  respect to its variants. The symbol that is used to express this is
  `^` and should be put in front of the trait you wish to be dispatched,
  e.g. `(T) where T: ^AsRef<str>`. The dispatcher is smart enough to
  figure out certain return types for methods such that non-matching
  variants can be assigned with a *default* return statement. i.e types
  like `Option<_>`, `Result<_, E>` and many other types (*including
  Primitive Types*) can get defaulted automatically for us instead of
  returning them with panic. *This is currently limited to rust std
  library traits, but there are plans to extend support for custom trait
  definitions soon.*

- **Impls** — can be seen as a shorthand for *a concrete type that
  implements this trait*, and are primarily used as a substitute for
  regular *generic trait bound expressions*. They look something like
  this, `(impl Copy, impl Copy) | {name: impl Clone}`

- **Placeholders** — are single unbounded wildcards, or if you are
  familiar with rust, it's the underscore `_` identifier and usually
  means that something is ignored, which means that they will satisfy
  any type `(_, _) | {num: _}`.

- **Variadic** — are similar to placeholders, but instead of only being
  able to substitute one type, variadics can be substituted by 0 or more
  types. Like placeholders, they are a way to express that we don't care
  about the rest of the parameters in a pattern. The look something like
  this`(T, U, ..) | {num: T, ..}`.

## Installation
This crate is available on [crates.io](https://crates.io/crates/penum)
and can be used by adding the following to your project's Cargo.toml:
```toml
[dependencies]
penum = "0.1.18"
```
Or run this command in your cargo project:
```sh
$ cargo add penum
```
## Overview 
A `Penum` expression can look like this:
```text
                      Dispatch symbol
                      |
#[penum( (T) where T: ^Trait )]
         ^^^       ^^^^^^^^^
         |         |
         |         Predicate bound
         |
         Pattern fragment.
```
*Note that there can be multiple patterns fragments and predicate
bounds.*

`Penum` is smart enough to infer certain return types for non-matching
variants. e.g `Option<T>`, `&Option<T>`, `String`, `&str`. It can even
handle `&String`, referenced non-const types. The goal is to support any
type, which we could potentially do by checking for types implementing
the `Default` trait.

Note, when dispatching traits with associated types, it's important to
declare them. e.g `Add<i32, Output = i32>`.

<details>
<summary>Under development</summary>

For non-std types we rely on the `Default` trait, which means, if we can
prove that a type implements `Default` we can automatically add them as
return types for non-matching variants,

</details>

### Trivial example:
Here we have an enum with one unary and one binary tuple variant where
the field type `Storage` and `Something` implements the trait `Trait`.
The goal is to be able to call the trait `method` through `Foo`. This
can be accomplished automatically by marking the trait with a dispatch
symbol `^`.
```rust
#[penum{ unit | (T) | (_, T) where T: ^Trait }]
enum Foo {
    V1(Storage), 
    V2(i32, Something), 
    V3
}
```

- Will turn into this:
```rust
impl Trait for Foo {
    fn method(&self, text: &str) -> &Option<&str> {
        match self {
            V1(val) => val.method(text),
            V2(_, val) => val.method(text),
            _ => &None
        }
    }
}
```

<details>
<summary>Boilerplate code for the example above</summary>

```rust
    struct Storage;
    struct Something;
    trait Trait {
        fn method(&self, text: &str) -> &Option<&str>;
    }
    impl Trait for Storage { ... }
    impl Trait for Something { ... }
```

</details>


## Examples
Used penum to force every variant to be a tuple with one field that must
implement `Copy`.

```rust
#[penum( (T, ..) where T: Copy )]
enum Guard {
    Bar(String), 
        ^^^^^^
    // ERROR: `String` doesn't implement `Copy`

    Bor(Option<String>), 
        ^^^^^^^^^^^^^^
    // ERROR: `Option<String>` doesn't implement `Copy`

    Bur(Vec<String>), 
        ^^^^^^^^^^^
    // ERROR: `Vec<String>` doesn't implement `Copy`

    Byr(), 
    ^^^^^
    // ERROR: `Byr()` doesn't match pattern `(T)`

    Bxr { name: usize }, 
        ^^^^^^^^^^^^^^^
    // ERROR: `{ nname: usize }` doesn't match pattern `(T)`

    Brr,
    ^^^
    // ERROR: `Brr` doesn't match pattern `(T)`

    Bir(i32, String), // Works!
    Beer(i32)         // Works!
}
```

#### Under development
- `Static dispatch` - auto implement `core`/`std`/`custom` traits ([read
  more](https://github.com/viktorlott/penum/blob/main/docs/static-dispatch.md)).


|   Traits   |   Supported   |
| ---------- | ------------- |
|`Any`| supported |
|`Borrow`| supported |
|`BorrowMut`| supported |
|`Eq`| supported |
|`AsMut`| supported |
|`AsRef`| supported |
|`From`| supported |
|`Into`| supported |
|`TryFrom`| supported |
|`TryInto`| supported |
|`Default`| supported |
|`Binary`| supported |
|`Debug`| supported |
|`Display`| supported |
|`LowerExp`| supported |
|`LowerHex`| supported |
|`Octal`| supported |
|`Pointer`| supported |
|`UpperExp`| supported |
|`UpperHex`| supported |
|`Future`| supported |
|`IntoFuture`| supported |
|`FromIterator`| supported |
|`FusedIterator`| supported |
|`IntoIterator`| supported |
|`Product`| supported |
|`Sum`| supported |
|`Copy`| supported |
|`Sized`| supported |
|`ToSocketAddrs`| supported |
|`Add`| supported |
|`AddAssign`| supported |
|`BitAnd`| supported |
|`BitAndAssign`| supported |
|`BitOr`| supported |
|`BitOrAssign`| supported |
|`BitXor`| supported |
|`BitXorAssign`| supported |
|`Deref`| supported |
|`DerefMut`| supported |
|`Div`| supported |
|`DivAssign`| supported |
|`Drop`| supported |
|`Fn`| supported |
|`FnMut`| supported |
|`FnOnce`| supported |
|`Index`| supported |
|`IndexMut`| supported |
|`Mul`| supported |
|`MulAssign`| supported |
|`MultiMethod`| supported |
|`Neg`| supported |
|`Not`| supported |
|`Rem`| supported |
|`RemAssign`| supported |
|`Shl`| supported |
|`ShlAssign`| supported |
|`Shr`| supported |
|`ShrAssign`| supported |
|`Sub`| supported |
|`SubAssign`| supported |
|`Termination`| supported |
|`SliceIndex`| supported |
|`FromStr`| supported |
|`ToString`| supported |