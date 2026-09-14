//! Source locations.
//!
//! An [`Origin`] is a byte range in a source file. Line and column numbers
//! are deliberately *not* stored: they are derivable from a byte offset and
//! the source text, and holding them cost 32 of an `Origin`'s 40 bytes — a
//! price every AST node, token, comment, and error paid, and on a
//! declaration-heavy file the AST was over 90% of the memory a compile
//! retained. Anything that shows a position to a person recovers it through
//! a [`LineIndex`].

use std::{collections::HashMap, fmt::Display};

use serde::Serialize;

pub type FileId = u32;

#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone, PartialOrd, Ord, Default)]
pub enum PositionKind {
    File(FileId),
    Builtin,
    #[default]
    Unknown, // Only used for the 'unknown' type.
}

/// The lexer's cursor: where it is, and in what.
///
/// Only ever a handful exist at a time, so unlike `Origin` this type is not
/// under size pressure.
#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone, PartialOrd, Ord)]
pub struct Position {
    pub byte_offset: u32,
    pub kind: PositionKind,
}

/// A byte range in a source file, `start..end`.
#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone, PartialOrd, Ord, Default)]
pub struct Origin {
    pub start: u32,
    // Exclusive.
    pub end: u32,
    pub kind: PositionKind,
}

pub struct OriginFormatter<'a, 'b> {
    origin: Origin,
    file_name: Option<&'a str>,
    line_index: &'b LineIndex<'b>,
}

/// Recovers one-based line and column numbers from a byte offset.
///
/// Building the table is a single pass over the source; a lookup is a binary
/// search over the line starts. Callers that render many positions — a run
/// of diagnostics, a language-server response — should build one and reuse
/// it rather than paying the pass per position.
pub struct LineIndex<'a> {
    input: &'a str,
    /// Byte offset of the first byte of each line. Always begins with 0, so
    /// it is never empty and a lookup always finds a line.
    line_starts: Vec<u32>,
}

impl<'a> LineIndex<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut line_starts = vec![0u32];
        line_starts.extend(
            input
                .bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| i as u32 + 1),
        );
        Self { input, line_starts }
    }

    pub fn input(&self) -> &'a str {
        self.input
    }

    /// The zero-based line containing `byte_offset`, and the byte offset of
    /// that line's first byte.
    fn line_of(&self, byte_offset: u32) -> (u32, u32) {
        // `partition_point` gives the count of line starts at or before the
        // offset, which is the one-based line; one less indexes the table.
        let line = self.line_starts.partition_point(|s| *s <= byte_offset) - 1;
        (line as u32, self.line_starts[line])
    }

    /// One-based line and column. The column counts bytes, matching the
    /// UTF-8 position encoding the language server negotiates.
    pub fn line_column(&self, byte_offset: u32) -> (u32, u32) {
        let (line, line_start) = self.line_of(byte_offset);
        (line + 1, byte_offset.saturating_sub(line_start) + 1)
    }

    /// Zero-based line and byte-within-line, as LSP positions are counted.
    pub fn line_character(&self, byte_offset: u32) -> (u32, u32) {
        let (line, line_start) = self.line_of(byte_offset);
        (line, byte_offset.saturating_sub(line_start))
    }

    /// The byte offset at which a zero-based line begins, or `None` past the
    /// end of the input.
    pub fn line_start(&self, line: u32) -> Option<u32> {
        self.line_starts.get(line as usize).copied()
    }
}

impl From<Origin> for std::ops::Range<usize> {
    fn from(origin: Origin) -> Self {
        origin.start as usize..origin.end as usize
    }
}

impl From<Position> for Origin {
    fn from(value: Position) -> Self {
        // Length is 0.
        Self {
            start: value.byte_offset,
            end: value.byte_offset,
            kind: value.kind,
        }
    }
}

impl<'a, 'b> Display for OriginFormatter<'a, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.origin.kind() {
            PositionKind::File(_) => {
                let file_name: &str = self.file_name.unwrap();
                f.write_str(file_name)
            }
            PositionKind::Builtin => f.write_str("builtin"),
            PositionKind::Unknown => f.write_str("unknown"),
        }?;
        let (start_line, start_column) = self.line_index.line_column(self.origin.start);
        let (end_line, end_column) = self.line_index.line_column(self.origin.end);
        write!(
            f,
            ":{}:{}:{}-{}:{}:{}",
            start_line, start_column, self.origin.start, end_line, end_column, self.origin.end,
        )
    }
}

impl Position {
    /// The origin running from this position up to — but not including —
    /// `to`. Callers pass the cursor after the text they lexed, so the
    /// resulting range covers exactly that text.
    pub(crate) fn extend_to(&self, to: Position) -> Origin {
        Origin {
            start: self.byte_offset,
            end: to.byte_offset,
            kind: self.kind,
        }
    }
}

impl Origin {
    pub fn display<'a, 'b>(
        &self,
        file_id_to_name: &'a HashMap<FileId, String>,
        line_index: &'b LineIndex<'b>,
    ) -> OriginFormatter<'a, 'b> {
        OriginFormatter {
            origin: *self,
            file_name: if let PositionKind::File(file_id) = self.kind() {
                file_id_to_name.get(&file_id).map(|s| s.as_str())
            } else {
                None
            },
            line_index,
        }
    }

    pub(crate) fn forwards(&self, skip_bytes: usize) -> Origin {
        let skip: u32 = skip_bytes.try_into().unwrap();
        Origin {
            start: self.start + skip,
            ..*self
        }
    }

    pub(crate) fn backwards(&self, skip_bytes: usize) -> Origin {
        let skip: u32 = skip_bytes.try_into().unwrap();
        Origin {
            end: self.end.checked_sub(skip).unwrap(),
            ..*self
        }
    }

    /// The sub-range of this origin beginning `skip` bytes in and running
    /// for `len` bytes.
    pub(crate) fn slice(&self, skip: u32, len: u32) -> Origin {
        Origin {
            start: self.start + skip,
            end: self.start + skip + len,
            ..*self
        }
    }

    /// This origin with its end moved to the byte offset `end`.
    pub(crate) fn extended_to(&self, end: u32) -> Origin {
        Origin { end, ..*self }
    }

    /// Extend this origin to cover `other`, returning a new `Origin` whose start is `self.start`
    /// and whose end is `other.end`.
    pub(crate) fn merge(&self, other: Origin) -> Origin {
        Origin {
            start: self.start,
            end: other.end,
            kind: self.kind,
        }
    }

    pub(crate) fn new_builtin() -> Origin {
        Origin {
            start: 0,
            end: 0,
            kind: PositionKind::Builtin,
        }
    }

    /// The kind of the origin.
    ///
    /// Kept as a method rather than a bare field read because it used to
    /// live on each end separately, and the two could disagree — error
    /// recovery could produce a `Builtin` start with a `File` end, which
    /// tripped an assertion here and aborted the process mid-diagnostic. An
    /// origin now carries one kind and the question cannot arise.
    pub fn kind(&self) -> PositionKind {
        self.kind
    }

    /// The length of the origin in bytes, or zero if its ends are inverted.
    ///
    /// Error recovery can produce an origin whose end precedes its start. A
    /// plain subtraction panicked in debug and, with overflow checks off in
    /// release, wrapped to roughly `usize::MAX` — which the hover handler
    /// silently took as the *smallest* enclosing node.
    pub fn len(&self) -> usize {
        (self.end as usize).saturating_sub(self.start as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(start: u32, end: u32, kind: PositionKind) -> Origin {
        Origin { start, end, kind }
    }

    #[test]
    fn test_len_of_an_inverted_origin_is_zero() {
        // Regression: error recovery can produce an origin whose end
        // precedes its start. The subtraction panicked in debug and wrapped
        // to roughly `usize::MAX` in release, which the hover handler then
        // took as the smallest enclosing node.
        assert_eq!(origin(10, 4, PositionKind::File(1)).len(), 0);
    }

    #[test]
    fn test_len_of_an_ordinary_origin() {
        assert_eq!(origin(4, 10, PositionKind::File(1)).len(), 6);
    }

    #[test]
    fn test_line_column_of_the_first_line() {
        let index = LineIndex::new("one\ntwo\n");
        assert_eq!(index.line_column(0), (1, 1));
        assert_eq!(index.line_column(2), (1, 3));
    }

    #[test]
    fn test_line_column_after_a_newline() {
        let index = LineIndex::new("one\ntwo\n");
        // The offset of the `\n` itself still belongs to the line it ends.
        assert_eq!(index.line_column(3), (1, 4));
        assert_eq!(index.line_column(4), (2, 1));
        assert_eq!(index.line_column(7), (2, 4));
    }

    #[test]
    fn test_line_column_past_the_end_of_the_input() {
        // Origins can end one past the last byte, and error recovery can
        // reach further still; neither may panic.
        let index = LineIndex::new("one\ntwo");
        assert_eq!(index.line_column(7), (2, 4));
        assert_eq!(index.line_column(9_999), (2, 9_996));
    }

    #[test]
    fn test_line_column_of_an_empty_input() {
        let index = LineIndex::new("");
        assert_eq!(index.line_column(0), (1, 1));
    }

    #[test]
    fn test_line_column_counts_columns_in_bytes() {
        // The language server negotiates UTF-8 position encoding, so a
        // multi-byte character advances the column by its byte length.
        let index = LineIndex::new("é=1");
        assert_eq!(index.line_column(0), (1, 1));
        assert_eq!(index.line_column(2), (1, 3));
    }

    #[test]
    fn test_line_character_is_zero_based() {
        let index = LineIndex::new("one\ntwo\n");
        assert_eq!(index.line_character(4), (1, 0));
        assert_eq!(index.line_character(6), (1, 2));
    }

    #[test]
    fn test_display_of_a_file_origin_spanning_two_lines() {
        // The rendered line and column come from the `LineIndex`, not from
        // the origin, which carries byte offsets alone.
        let input = "one\ntwo\nthree\n";
        let index = LineIndex::new(input);
        let mut names = HashMap::new();
        names.insert(1, String::from("t.d"));
        let origin = Origin {
            start: 5,
            end: 10,
            kind: PositionKind::File(1),
        };
        assert_eq!(
            origin.display(&names, &index).to_string(),
            "t.d:2:2:5-3:3:10"
        );
    }

    #[test]
    fn test_display_of_a_builtin_origin_names_no_file() {
        let index = LineIndex::new("");
        let names = HashMap::new();
        assert_eq!(
            Origin::new_builtin().display(&names, &index).to_string(),
            "builtin:1:1:0-1:1:0"
        );
    }

    #[test]
    fn test_display_of_an_unknown_origin_names_no_file() {
        let index = LineIndex::new("");
        let names = HashMap::new();
        let origin = Origin::default();
        assert_eq!(
            origin.display(&names, &index).to_string(),
            "unknown:1:1:0-1:1:0"
        );
    }

    #[test]
    fn test_slice_is_relative_to_the_start_of_the_origin() {
        let origin = Origin {
            start: 10,
            end: 20,
            kind: PositionKind::File(1),
        };
        let sliced = origin.slice(3, 4);
        assert_eq!(sliced.start, 13);
        assert_eq!(sliced.end, 17);
        assert_eq!(sliced.kind, PositionKind::File(1));
    }

    #[test]
    fn test_extend_to_covers_the_text_between_two_positions() {
        // The end is exclusive: callers pass the cursor sitting after the
        // text they lexed, and the origin must cover exactly that text.
        let input = "abcdef";
        let at = |byte_offset| Position {
            byte_offset,
            kind: PositionKind::File(1),
        };
        let origin = at(1).extend_to(at(4));
        assert_eq!(&input[std::ops::Range::<usize>::from(origin)], "bcd");
        assert_eq!(origin.len(), 3);
    }

    #[test]
    fn test_line_start() {
        let index = LineIndex::new("one\ntwo\n");
        assert_eq!(index.line_start(0), Some(0));
        assert_eq!(index.line_start(1), Some(4));
        assert_eq!(index.line_start(2), Some(8));
        assert_eq!(index.line_start(3), None);
    }
}
