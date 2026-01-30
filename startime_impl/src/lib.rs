use proc_macro2::{Span, TokenStream};
use quote::quote_spanned;
use starlark::PrintHandler;
use starlark::environment::{Globals, GlobalsBuilder, LibraryExtension, Module};
use starlark::eval::Evaluator;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::Value;

mod content_map;

#[proc_macro]
pub fn startime(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let (content, tree) = content_map::build(input.into());
    startime_impl(content)
        .unwrap_or_else(|e| match e {
            Error::Starlark(error) => {
                let message = error.to_string();
                let span = if let Some(file_span) = error.span() {
                    tree.starlark_offset_to_rust_span(file_span.span.begin().get() as usize)
                } else {
                    Span::call_site()
                };
                quote_spanned! {span=>
                    compile_error!(#message);
                }
            }
            Error::TokenStream(lex_error) => {
                let message = lex_error.to_string();
                let span = Span::call_site();
                quote_spanned! {span=>
                    compile_error!(#message);
                }
            }
            Error::NoString => {
                let span = Span::call_site();
                quote_spanned! {span=>
                    compile_error!("Didn't return a string");
                }
            }
        })
        .into()
}

enum Error {
    Starlark(starlark::Error),
    TokenStream(proc_macro2::LexError),
    NoString,
}

fn startime_impl(content: String) -> Result<TokenStream, Error> {
    let ast: AstModule = AstModule::parse(
        &Span::call_site().file(),
        content,
        &Dialect {
            enable_top_level_stmt: true,
            ..Default::default()
        },
    )
    .map_err(Error::Starlark)?;

    // We create a `Globals`, defining the standard library functions available.
    // The `standard` function uses those defined in the Starlark specification.
    let globals: Globals = GlobalsBuilder::standard()
        .with(|b| LibraryExtension::Print.add(b))
        .build();

    // We create a `Module`, which stores the global variables for our calculation.
    let module: Module = Module::new();

    let print_handler = PrefixPrinter {
        prefix: {
            let callsite = Span::call_site();
            format!(
                "[{}:{}:{}] ",
                callsite.file(),
                callsite.start().line,
                callsite.start().column
            )
        },
    };

    // We create an evaluator, which controls how evaluation occurs.
    let mut eval: Evaluator = Evaluator::new(&module);
    eval.set_print_handler(&print_handler);

    // And finally we evaluate the code using the evaluator.
    let res: Value = eval.eval_module(ast, &globals).map_err(Error::Starlark)?;

    res.unpack_str()
        .ok_or(Error::NoString)?
        .parse()
        .map_err(Error::TokenStream)
}

struct PrefixPrinter {
    prefix: String,
}

impl PrintHandler for PrefixPrinter {
    fn println(&self, text: &str) -> starlark::Result<()> {
        eprintln!("{}{text}", self.prefix);
        Ok(())
    }
}
