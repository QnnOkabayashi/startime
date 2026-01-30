use proc_macro2::{Delimiter, LineColumn, Span, TokenStream, TokenTree};

pub fn build(tokens: TokenStream) -> (String, Tree) {
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
pub struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    pub fn starlark_offset_to_rust_span(&self, offset: usize) -> Span {
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
        if loc >= Span::call_site().end() {
            // If the location comes from outside of this callsite, do not add whitespace.
            return;
        }
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
