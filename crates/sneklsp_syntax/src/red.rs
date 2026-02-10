use std::sync::Arc;

use crate::SyntaxKind;
use crate::green::{GreenChild, GreenNode};
use sneklsp_text::{TextRange, TextSize};

#[derive(Debug, Clone)]
pub struct SyntaxNode {
    green: GreenNode,
    offset: TextSize,
}

impl SyntaxNode {
    pub fn new_root(green: GreenNode) -> Self {
        Self {
            green,
            offset: TextSize::new(0),
        }
    }

    #[inline]
    pub fn kind(&self) -> SyntaxKind {
        self.green.kind()
    }

    #[inline]
    pub fn text_range(&self) -> TextRange {
        TextRange::new(self.offset, self.offset + self.green.text_len())
    }

    #[inline]
    pub fn text_len(&self) -> TextSize {
        self.green.text_len()
    }

    pub fn children(&self) -> impl Iterator<Item = SyntaxElement> + '_ {
        let mut offset = self.offset;
        self.green.children().iter().map(move |child| {
            let child_offset = offset;
            offset = offset + child.text_len();
            match child {
                GreenChild::Token(token) => SyntaxElement::Token(SyntaxToken {
                    kind: token.kind,
                    text: Arc::clone(&token.text),
                    offset: child_offset,
                }),
                GreenChild::Node(node) => SyntaxElement::Node(SyntaxNode {
                    green: node.clone(),
                    offset: child_offset,
                }),
            }
        })
    }

    pub fn token_at_offset(&self, offset: TextSize) -> Option<SyntaxToken> {
        if !self.text_range().contains(offset) {
            return None;
        }

        for child in self.children() {
            match child {
                SyntaxElement::Token(token) => {
                    if token.text_range().contains(offset) {
                        return Some(token);
                    }
                }
                SyntaxElement::Node(node) => {
                    if let Some(token) = node.token_at_offset(offset) {
                        return Some(token);
                    }
                }
            }
        }

        None
    }

    #[inline]
    pub fn green(&self) -> &GreenNode {
        &self.green
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxToken {
    pub kind: SyntaxKind,
    pub text: Arc<str>,
    pub offset: TextSize,
}

impl SyntaxToken {
    #[inline]
    pub fn text_range(&self) -> TextRange {
        TextRange::new(
            self.offset,
            self.offset + TextSize::new(self.text.len() as u32),
        )
    }
}

#[derive(Debug, Clone)]
pub enum SyntaxElement {
    Node(SyntaxNode),
    Token(SyntaxToken),
}

impl SyntaxElement {
    pub fn text_range(&self) -> TextRange {
        match self {
            SyntaxElement::Node(n) => n.text_range(),
            SyntaxElement::Token(t) => t.text_range(),
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        match self {
            SyntaxElement::Node(n) => n.kind(),
            SyntaxElement::Token(t) => t.kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::green::GreenNodeBuilder;

    #[test]
    fn red_tree_offsets() {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Module);
        builder.token(SyntaxKind::Name, "hello");
        builder.token(SyntaxKind::Whitespace, " ");
        builder.token(SyntaxKind::Eq, "=");
        builder.token(SyntaxKind::Whitespace, " ");
        builder.token(SyntaxKind::Int, "42");
        let green = builder.finish();

        let root = SyntaxNode::new_root(green);
        assert_eq!(
            root.text_range(),
            TextRange::new(TextSize::new(0), TextSize::new(10))
        );

        let children: Vec<_> = root.children().collect();
        assert_eq!(children.len(), 5);
        assert_eq!(
            children[4].text_range(),
            TextRange::new(TextSize::new(8), TextSize::new(10))
        );
    }

    #[test]
    fn token_at_offset() {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Module);
        builder.token(SyntaxKind::Name, "x");
        builder.token(SyntaxKind::Eq, "=");
        builder.token(SyntaxKind::Int, "1");
        let green = builder.finish();

        let root = SyntaxNode::new_root(green);

        let token = root.token_at_offset(TextSize::new(0)).unwrap();
        assert_eq!(token.kind, SyntaxKind::Name);

        let token = root.token_at_offset(TextSize::new(1)).unwrap();
        assert_eq!(token.kind, SyntaxKind::Eq);

        let token = root.token_at_offset(TextSize::new(2)).unwrap();
        assert_eq!(token.kind, SyntaxKind::Int);
    }
}
