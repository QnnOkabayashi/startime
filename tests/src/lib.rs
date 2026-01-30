// startime::startime! {
//     output = ""
//     for name in ["Foo", "Bar"]:
//         output += """
//         struct {};
//         """.format(name)

//     output
// }

// fn _foo(_: Foo) {}
// fn _bar(_: Bar) {}

macro_rules! gen_positions {
    ($components:expr) => {
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

pub fn test() {
    let _ = Position1::X;
    let _ = Position2::Y;
}
