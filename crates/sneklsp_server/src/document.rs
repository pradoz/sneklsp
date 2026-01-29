use sneklsp_parser::ParseError;
use sneklsp_text::LineIndex;

#[derive(Debug)]
pub struct Document {
    pub content: String,
    pub line_index: LineIndex,
    pub errors: Vec<ParseError>,
    pub version: i32,
}

impl Document {
    pub fn new(content: String, version: i32) -> Self {
        let line_index = LineIndex::new(&content);
        let errors = Self::parse_content(&content);
        Self {
            content,
            line_index,
            errors,
            version,
        }
    }

    pub fn update(&mut self, content: String, version: i32) {
        self.content = content;
        self.version = version;
        self.line_index = LineIndex::new(&self.content);
        self.errors = Self::parse_content(&self.content);
    }

    fn parse_content(content: &str) -> Vec<ParseError> {
        sneklsp_parser::parse_and_collect_errors(content)
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}
