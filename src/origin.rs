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

#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub byte_offset: u32,
    pub kind: PositionKind,
}

#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone, PartialOrd, Ord, Default)]
pub struct Origin {
    pub start: Position,
    // Inclusive.
    pub end: Position,
}

pub struct OriginFormatter<'a> {
    origin: Origin,
    file_name: Option<&'a str>,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            line: 1,
            column: 1,
            byte_offset: 0,
            kind: Default::default(),
        }
    }
}

impl From<Origin> for std::ops::Range<usize> {
    fn from(origin: Origin) -> Self {
        origin.start.byte_offset as usize..origin.end.byte_offset as usize
    }
}

impl From<Position> for Origin {
    fn from(value: Position) -> Self {
        // Length is 0.
        Self {
            start: value,
            end: value,
        }
    }
}

impl<'a> Display for OriginFormatter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.origin.kind() {
            PositionKind::File(_) => {
                let file_name: &str = self.file_name.unwrap();
                f.write_str(file_name)
            }
            PositionKind::Builtin => f.write_str("builtin"),
            PositionKind::Unknown => f.write_str("unknown"),
        }?;
        write!(
            f,
            ":{}:{}:{}",
            self.origin.start.line, self.origin.start.column, self.origin.start.byte_offset
        )
    }
}

impl Position {
    pub(crate) fn extend_to_inclusive(&self, to: Position) -> Origin {
        Origin {
            start: *self,
            end: to,
        }
    }
}

impl Origin {
    pub(crate) fn display<'a>(
        &self,
        file_id_to_name: &'a HashMap<FileId, String>,
    ) -> OriginFormatter<'a> {
        OriginFormatter {
            origin: *self,
            file_name: if let PositionKind::File(file_id) = self.kind() {
                file_id_to_name.get(&file_id).map(|s| s.as_str())
            } else {
                None
            },
        }
    }

    pub(crate) fn forwards(&self, skip_bytes: usize) -> Origin {
        let skip: u32 = skip_bytes.try_into().unwrap();
        Origin {
            start: Position {
                byte_offset: self.start.byte_offset + skip,
                column: self.start.column + skip,
                // Assume that the line remains the same.
                ..self.start
            },
            ..*self
        }
    }

    pub(crate) fn backwards(&self, skip_bytes: usize) -> Origin {
        let skip: u32 = skip_bytes.try_into().unwrap();
        Origin {
            end: Position {
                byte_offset: self.end.byte_offset.checked_sub(skip).unwrap(),
                column: self.end.column.checked_sub(skip).unwrap(),
                // Assume that the line remains the same.
                ..self.end
            },
            ..*self
        }
    }

    /// Extend this origin to cover `other`, returning a new `Origin` whose start is `self.start`
    /// and whose end is `other.end`.
    pub(crate) fn merge(&self, other: Origin) -> Origin {
        Origin {
            start: self.start,
            end: other.end,
        }
    }

    pub(crate) fn new_builtin() -> Origin {
        Origin {
            start: Position {
                kind: PositionKind::Builtin,
                ..Default::default()
            },
            end: Position {
                kind: PositionKind::Builtin,
                ..Default::default()
            },
        }
    }

    pub fn kind(&self) -> PositionKind {
        assert_eq!(self.start.kind, self.end.kind);
        self.start.kind
    }

    pub fn len(&self) -> usize {
        self.end.byte_offset as usize - self.start.byte_offset as usize
    }
}
