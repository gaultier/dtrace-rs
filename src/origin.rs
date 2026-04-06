use std::{collections::HashMap, fmt::Display};

use serde::Serialize;

pub type FileId = u32;

#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone, PartialOrd, Ord)]
pub enum OriginKind {
    File(FileId),
    Builtin,
    Unknown, // Only used for the 'unknown' type.
}

#[derive(PartialEq, Eq, Debug, Serialize, Copy, Clone, PartialOrd, Ord)]
pub struct Origin {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
    pub len: u32,
    pub kind: OriginKind,
}

pub struct OriginFormatter<'a> {
    origin: Origin,
    file_name: Option<&'a str>,
}

impl<'a> Display for OriginFormatter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.origin.kind {
            OriginKind::File(_) => {
                let file_name: &str = self.file_name.unwrap();
                f.write_str(file_name)
            }
            OriginKind::Builtin => f.write_str("builtin"),
            OriginKind::Unknown => f.write_str("unknown"),
        }?;
        write!(
            f,
            ":{}:{}:{}",
            self.origin.line, self.origin.column, self.origin.offset
        )
    }
}

impl Origin {
    pub(crate) fn display<'a>(
        &self,
        file_id_to_name: &'a HashMap<FileId, String>,
    ) -> OriginFormatter<'a> {
        OriginFormatter {
            origin: *self,
            file_name: if let OriginKind::File(file_id) = self.kind {
                file_id_to_name.get(&file_id).map(|s| s.as_str())
            } else {
                None
            },
        }
    }

    pub(crate) fn extends_to(&self, to: Option<Origin>) -> Origin {
        if let Some(to) = to {
            assert!(to.offset >= self.offset);

            Origin {
                len: to.offset + to.len - self.offset,
                ..*self
            }
        } else {
            *self
        }
    }

    pub(crate) fn with_len(&self, len: usize) -> Origin {
        Origin {
            len: len.try_into().unwrap(),
            ..*self
        }
    }

    pub(crate) fn skip(&self, skip: usize) -> Origin {
        let skip: u32 = skip.try_into().unwrap();
        Origin {
            offset: self.offset + skip,
            len: self.len - skip,
            column: self.column + skip,
            ..*self
        }
    }

    pub(crate) fn new(line: u32, column: u32, offset: u32, len: u32, file_id: FileId) -> Self {
        Self {
            line,
            column,
            offset,
            len,
            kind: OriginKind::File(file_id),
        }
    }

    pub(crate) fn new_builtin() -> Self {
        Origin {
            line: 0,
            column: 0,
            offset: 0,
            len: 0,
            kind: OriginKind::Builtin,
        }
    }

    pub(crate) fn new_unknown() -> Self {
        Origin {
            line: 0,
            column: 0,
            offset: 0,
            len: 0,
            kind: OriginKind::Unknown,
        }
    }
}
