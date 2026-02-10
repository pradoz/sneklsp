use std::sync::Arc;

use crate::SyntaxKind;
use sneklsp_text::TextSize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GreenToken {
    pub kind: SyntaxKind,
    pub text: Arc<str>,
}

impl GreenToken {
    #[inline]
    pub fn new(kind: SyntaxKind, text: &str) -> Self {
        Self {
            kind,
            text: Arc::from(text),
        }
    }

    #[inline]
    pub fn text_len(&self) -> TextSize {
        TextSize::new(self.text.len() as u32)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GreenNode {
    kind: SyntaxKind,
    children: Arc<[GreenChild]>,
    text_len: TextSize,
}

impl GreenNode {
    pub fn new(kind: SyntaxKind, children: Vec<GreenChild>) -> Self {
        let text_len = children
            .iter()
            .map(|c| c.text_len())
            .fold(TextSize::new(0), |acc, len| acc + len);
        Self {
            kind,
            children: children.into(),
            text_len,
        }
    }

    #[inline]
    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    #[inline]
    pub fn children(&self) -> &[GreenChild] {
        &self.children
    }

    #[inline]
    pub fn text_len(&self) -> TextSize {
        self.text_len
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GreenChild {
    Token(GreenToken),
    Node(GreenNode),
}

impl GreenChild {
    pub fn text_len(&self) -> TextSize {
        match self {
            GreenChild::Token(t) => t.text_len(),
            GreenChild::Node(n) => n.text_len(),
        }
    }
}

pub struct GreenNodeBuilder {
    stack: Vec<(SyntaxKind, Vec<GreenChild>)>,
}

impl GreenNodeBuilder {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn start_node(&mut self, kind: SyntaxKind) {
        self.stack.push((kind, Vec::new()));
    }

    pub fn finish_node(&mut self) {
        let (kind, children) = self.stack.pop().expect("unbalanced start/finish");
        let node = GreenNode::new(kind, children);

        if let Some((_, parent_children)) = self.stack.last_mut() {
            parent_children.push(GreenChild::Node(node));
        } else {
            self.stack.push((kind, vec![GreenChild::Node(node)]));
        }
    }

    pub fn token(&mut self, kind: SyntaxKind, text: &str) {
        let token = GreenToken::new(kind, text);
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(GreenChild::Token(token));
        }
    }

    pub fn finish(&mut self) -> GreenNode {
        assert_eq!(self.stack.len(), 1, "unbalanced tree");
        let (kind, children) = self.stack.pop().unwrap();
        GreenNode::new(kind, children)
    }
}

impl Default for GreenNodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_tree() {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Module);
        builder.start_node(SyntaxKind::ExprStmt);
        builder.token(SyntaxKind::Int, "42");
        builder.finish_node();
        builder.token(SyntaxKind::Newline, "\n");
        let root = builder.finish();

        assert_eq!(root.kind(), SyntaxKind::Module);
        assert_eq!(root.children().len(), 2);
        assert_eq!(root.text_len(), TextSize::new(3));
    }
}
