use std::{collections::HashMap, io::Write};

use serde::Serialize;

use crate::{
    lex::TokenKind,
    origin::{FileId, LineIndex, Origin},
    type_checker::Type,
};

#[derive(Serialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum ErrorKind {
    UnknownToken,
    InvalidLiteralNumber,
    InvalidLiteralString,
    InvalidLiteralCharacter,
    MissingProbeSpecifier,
    MissingPredicateOrAction,
    ParseProgram,
    ParseStatement,
    IncompatibleTypes,
    IncompatibleArgumentsCount,
    UnknownIdentifier,
    CallingANonFunction,
    MissingExpectedToken(TokenKind),
    MissingExpr,
    MissingArguments,
    NameAlreadyDefined,
    EmptyTranslationUnit,
    MissingFieldOrKeywordInMemberAccess,
    MissingStatementOrBlock,
    MissingStatement,
    MissingTypeName,
    MissingDirectDeclarator,
    MissingInitDeclarator,
    MissingEnumerators,
    MissingEnumerator,
    MissingStructDeclarationList,
    MissingConstantExpr,
    MissingStructFieldDeclarator,
    MissingAbstractDeclarator,
    MissingArray,
    MissingFunction,
    MissingDeclarator,
    MissingArrayParameters,
    MissingParameterDeclarationSpecifiers,
    MissingDeclarationSpecifiers,
    InvalidControlDirective,
    MissingFunctionParameter,
    MissingFunctionParameters,
    InvalidVersionString,
    InvalidStability,
    InvalidClass,
    NestedComment,
    UnterminatedComment,
    UnsupportedLiteralFloatNumber,
    UnexpectedPeriod,
    ShebangMustComeFirst,
    InvalidMacroArgumentReference,
    InvalidMacroArgument,
    MissingExprOrTypename,
    Redeclaration,
    NestingTooDeep,
}

#[derive(Serialize, Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub origin: Origin,
    pub explanation: String,
    pub related_origin: Option<Origin>,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, origin: Origin, explanation: String) -> Self {
        Self {
            kind,
            origin,
            explanation,
            related_origin: None,
        }
    }

    pub(crate) fn new_incompatible_types(origin: &Origin, a: &Type, b: &Type) -> Self {
        Self {
            kind: crate::error::ErrorKind::IncompatibleTypes,
            origin: *origin,
            explanation: format!("incompatible types: {} vs {}", a, b),
            related_origin: None,
        }
    }

    pub(crate) fn new_incompatible_arguments_count(
        origin: &Origin,
        expected: usize,
        found: usize,
    ) -> Self {
        Self {
            kind: crate::error::ErrorKind::IncompatibleArgumentsCount,
            origin: *origin,
            explanation: format!(
                "incompatible arguments count: expected {}, got {}",
                expected, found
            ),
            related_origin: None,
        }
    }

    // FIXME: Display.
    pub fn write<W: Write>(
        &self,
        w: &mut W,
        line_index: &LineIndex,
        file_id_to_name: &HashMap<FileId, String>,
    ) -> std::io::Result<()> {
        let input = line_index.input();
        write!(
            w,
            "{}: Error {:?}",
            self.origin.display(file_id_to_name, line_index),
            self.kind,
        )?;
        if !self.explanation.is_empty() {
            write!(w, ": {}", self.explanation)?;
        }
        w.write_all(b": ")?;

        write_excerpt(w, input, self.origin)?;

        if let Some(related_origin) = self.related_origin {
            write!(
                w,
                "\nHere: {}: ",
                related_origin.display(file_id_to_name, line_index)
            )?;

            write_excerpt(w, input, related_origin)?;
        }

        Ok(())
    }
}

pub fn write_excerpt<W: Write>(w: &mut W, input: &str, origin: Origin) -> std::io::Result<()> {
    let start = origin.start as usize;
    let end = origin.end as usize;

    // TODO: limit context length.
    let mut excerpt_start = start;
    while excerpt_start > 0 {
        excerpt_start -= 1;
        if input.as_bytes()[excerpt_start] == b'\n' {
            excerpt_start += 1;
            break;
        }
    }

    let mut excerpt_end = end;
    while excerpt_end < input.len() && input.as_bytes()[excerpt_end] != b'\n' {
        excerpt_end += 1;
    }

    let excerpt_before = &input[excerpt_start..start].trim_ascii_start();
    let excerpt = &input[start..end];
    let excerpt_after = &input[end..excerpt_end].trim_ascii_end();

    w.write_all(excerpt_before.as_bytes())?;
    w.write_all(b"\x1B[4m")?;
    w.write_all(excerpt.as_bytes())?;
    w.write_all(b"\x1B[0m")?;
    w.write_all(excerpt_after.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE_ID: u32 = 1;

    fn excerpt(input: &str, start: usize, end: usize) -> String {
        let origin = Origin {
            start: start as u32,
            end: end as u32,
            kind: crate::origin::PositionKind::File(FILE_ID),
        };
        let mut buf = Vec::new();
        write_excerpt(&mut buf, input, origin).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn test_write_excerpt_without_a_trailing_newline() {
        // Regression: the loop incremented before indexing, so an origin
        // ending at the last byte of a file with no trailing newline read
        // `input[input.len()]`.
        assert_eq!(excerpt("abc", 0, 3), "\x1B[4mabc\x1B[0m");
        assert_eq!(excerpt("abc", 2, 3), "ab\x1B[4mc\x1B[0m");
    }

    #[test]
    fn test_write_excerpt_of_an_empty_origin_at_the_end_of_the_input() {
        assert_eq!(excerpt("abc", 3, 3), "abc\x1B[4m\x1B[0m");
        assert_eq!(excerpt("", 0, 0), "\x1B[4m\x1B[0m");
    }

    #[test]
    fn test_write_excerpt_is_limited_to_the_origin_line() {
        let input = "one\ntwo\nthree\n";
        assert_eq!(excerpt(input, 4, 7), "\x1B[4mtwo\x1B[0m");
    }
}
