startime::startime! {
    output = ""
    for name in ["Foo", "Bar"]:
        output += """
        struct {};
        """.format(name)

    output
}

fn _foo(_: Foo) {}
fn _bar(_: Bar) {}
