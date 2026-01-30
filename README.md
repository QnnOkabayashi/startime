# Startime

Startime lets you run Starlark code at compile time to generate Rust code.

```rust
startime::startime! {
    output = ""
    for name in ["Foo", "Bar"]:
        output += """
        struct {};
        """.format(name)

    output
}

// Generates the following:

struct Foo;
struct Bar;
```

The executed Starlark code takes no inputs and is expected to produce a single `str` output.
However, you can nest it within declarative macros to generate `startime! {}` invocations with user provided arguments.

```rust
macro_rules! gen_positions {
    ($components:tt) => {
        startime::startime! {
            components = $components
            output = ""
            for ix in range(len(components)):
                dim = ix + 1
                cons = ",".join(components[:dim])
                output += """
                enum Position{} {{
                    {}
                }}
                """.format(dim, cons)
            output
        }
    };
}

gen_positions!(["X", "Y", "Z", "W"]);

// Generates the following

enum Position1 { X }
enum Position2 { X, Y }
enum Position3 { X, Y, Z }
enum Position4 { X, Y, Z, W }
```

## Known issues

When `startime! {}` is nested in a declarative macro, pasting in repeated elements from the declarative macro input will break `startime`s ability to recover the original source code.
