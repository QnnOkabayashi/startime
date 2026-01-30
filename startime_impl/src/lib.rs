use proc_macro2::{Delimiter, LineColumn, Span, TokenStream, TokenTree};
use quote::quote_spanned;
use starlark::environment::{Globals, Module};
use starlark::eval::Evaluator;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::Value;

#[proc_macro]
pub fn startime(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    startime_impl(input.into())
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn startime_impl(input: TokenStream) -> Result<TokenStream, Error> {
    let (content, tree) = token_stream_to_string_with_whitespace(input);

    let filename = Span::call_site().file();

    let mut dialect = Dialect::Standard;
    dialect.enable_top_level_stmt = true;

    let ast: AstModule = match AstModule::parse(&filename, content, &dialect) {
        Ok(ast) => ast,
        Err(e) => return Err(Error::Starlark(e, tree)),
    };

    // We create a `Globals`, defining the standard library functions available.
    // The `standard` function uses those defined in the Starlark specification.
    let globals: Globals = Globals::standard();

    // We create a `Module`, which stores the global variables for our calculation.
    let module: Module = Module::new();

    // We create an evaluator, which controls how evaluation occurs.
    let mut eval: Evaluator = Evaluator::new(&module);

    // And finally we evaluate the code using the evaluator.
    let res: Value = match eval.eval_module(ast, &globals) {
        Ok(res) => res,
        Err(e) => return Err(Error::Starlark(e, tree)),
    };
    let string = res.unpack_str().ok_or(Error::NoString)?;
    match string.parse::<TokenStream>() {
        Ok(ok) => {
            let span = Span::call_site();
            Ok(quote_spanned! {span=> #ok })
        }
        Err(err) => Err(Error::TokenStream(err)),
    }
}

fn token_stream_to_string_with_whitespace(tokens: TokenStream) -> (String, Tree) {
    let mut source = Source::default();
    source.reconstruct_from(tokens);
    (source.source, source.tree)
}

#[derive(Default)]
struct Source {
    source: String,
    start_col: Option<usize>,
    line: usize,
    col: usize,
    tree: Tree,
}

#[derive(Debug)]
struct Node {
    starlark_span: (usize, usize),
    rust_span: Span,
    subtree_size: usize,
}

#[derive(Default, Debug)]
struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    fn starlark_offset_to_rust_span(&self, offset: usize) -> Span {
        let mut i = 0;
        while offset > self.nodes[i].starlark_span.0 {
            // After the start of this span

            let curr = &self.nodes[i];
            // Also: after the end of this span
            i += 1;
            if offset > curr.starlark_span.1 {
                i += curr.subtree_size;
            }
        }
        self.nodes[i].rust_span
    }
}

impl Source {
    fn reconstruct_from(&mut self, input: TokenStream) {
        for tt in input {
            match tt {
                // Do not treat pasted-in tokens as groups
                TokenTree::Group(g) if g.delimiter() != Delimiter::None => {
                    // Reserve a placeholder node, because we need to push the
                    // subtree before we can populate this node.
                    let node_index = self.tree.nodes.len();
                    self.tree.nodes.push(Node {
                        starlark_span: (0, 0),
                        rust_span: Span::call_site(),
                        subtree_size: 0,
                    });

                    let s = g.to_string();
                    self.add_whitespace(g.span_open().start());
                    let start = self.source.len();
                    self.add_str(&s[..1]); // the '[', '{' or '('.

                    self.reconstruct_from(g.stream());
                    self.add_whitespace(g.span_close().start());
                    self.add_str(&s[s.len() - 1..]); // the ']', '}' or ')'.
                    let subtree_size = self.tree.nodes.len() - node_index;
                    let end = self.source.len();
                    self.tree.nodes[node_index] = Node {
                        starlark_span: (start, end),
                        rust_span: g.span(),
                        subtree_size,
                    };
                }
                _ => {
                    self.add_whitespace(tt.span().start());
                    let start = self.source.len();
                    // The token we're putting in the startime macro is coming from somewhere
                    // far away, we need to get where it appears in this macro invocation.
                    self.add_str(&tt.to_string());
                    let end = self.source.len();
                    self.tree.nodes.push(Node {
                        starlark_span: (start, end),
                        rust_span: tt.span(),
                        subtree_size: 0,
                    });
                }
            }
        }
    }

    fn add_str(&mut self, s: &str) {
        let mut parts = s.split_inclusive('\n');
        if let Some(part) = parts.next() {
            self.source += part;
            self.col += s.len();
        }
        let start_col = self
            .start_col
            .expect("should succeed because we called add_whitespace first");
        // Handle the rest of the multiline string literal
        for part in parts {
            self.line += 1;
            let dedented = &part[start_col..];
            self.source += dedented;
            self.col = dedented.len();
        }
    }

    fn add_whitespace(&mut self, loc: LineColumn) {
        while self.line < loc.line - 1 {
            self.source.push('\n');
            self.line += 1;
            self.col = 0;
        }
        let start_col = *self.start_col.get_or_insert(loc.column);
        let col = loc
            .column
            .checked_sub(start_col)
            .expect("Invalid indentation");
        while self.col < col {
            self.source.push(' ');
            self.col += 1;
        }
    }
}

enum Error {
    Starlark(starlark::Error, Tree),
    TokenStream(proc_macro2::LexError),
    NoString,
}

impl Error {
    fn into_compile_error(self) -> TokenStream {
        match self {
            Error::Starlark(error, tree) => {
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
        }
    }
}
