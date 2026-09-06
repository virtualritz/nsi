//! The lexer.
//!
//! ɴsɪ's stream format has no published grammar, so this is written
//! against what 3Delight 2.9.208 accepts. The decisive observation is
//! that an entire scene on one line parses, so newlines and indents are
//! formatting rather than syntax; see `specs/004-nsi-parse` D2.

use core::str;
use std::borrow::Cow;

/// One lexical token, borrowed from the input.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token<'a> {
    /// A bare token: a statement keyword, or a number in operand
    /// position.
    Word(&'a str),
    /// A string literal, already unescaped.
    ///
    /// Borrowed when it contained no escape, which is the common case
    /// and the reason this is not a `String`.
    Quoted(Quoted<'a>),
    /// `[`.
    Open,
    /// `]`.
    Close,
}

/// A string literal's text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Quoted<'a> {
    /// No escapes: a slice of the input.
    Borrowed(&'a str),
    /// Escapes were decoded.
    Owned(String),
}

impl<'a> Quoted<'a> {
    /// The text, still borrowed from the input where it was.
    pub(crate) fn into_cow(self) -> Cow<'a, str> {
        match self {
            Self::Borrowed(text) => Cow::Borrowed(text),
            Self::Owned(text) => Cow::Owned(text),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Borrowed(text) => text,
            Self::Owned(text) => text,
        }
    }
}

/// Why lexing stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LexError {
    /// A string literal ran to end of input.
    UnterminatedString { offset: usize },
    /// A byte sequence that is not UTF-8.
    NotUtf8 { offset: usize },
    /// An escape this format does not define.
    BadEscape { offset: usize },
}

/// A cursor over stream bytes.
pub(crate) struct Lexer<'a> {
    input: &'a [u8],
    position: usize,
    /// Where the token last returned began, after trivia. An error that
    /// points at the preceding whitespace is no better than one with no
    /// offset at all.
    token_start: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) const fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            position: 0,
            token_start: 0,
        }
    }

    /// Where the token last returned began.
    pub(crate) const fn offset(&self) -> usize {
        self.token_start
    }

    /// Skip whitespace and `#` comments.
    fn skip_trivia(&mut self) {
        loop {
            while self
                .input
                .get(self.position)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.position += 1;
            }

            if self.input.get(self.position) == Some(&b'#') {
                // A comment runs to end of line.
                self.position =
                    memchr::memchr(b'\n', &self.input[self.position..])
                        .map_or(self.input.len(), |end| self.position + end);
            } else {
                break;
            }
        }
    }

    /// The next token, or `None` at end of input.
    pub(crate) fn next_token(&mut self) -> Result<Option<Token<'a>>, LexError> {
        self.skip_trivia();
        self.token_start = self.position;

        let Some(&byte) = self.input.get(self.position) else {
            return Ok(None);
        };

        match byte {
            b'[' => {
                self.position += 1;
                Ok(Some(Token::Open))
            }
            b']' => {
                self.position += 1;
                Ok(Some(Token::Close))
            }
            b'"' => self.string().map(Some),
            _ => self.word().map(Some),
        }
    }

    /// A bare token, up to the next whitespace, bracket or quote.
    fn word(&mut self) -> Result<Token<'a>, LexError> {
        let start = self.position;
        while let Some(&byte) = self.input.get(self.position) {
            if byte.is_ascii_whitespace()
                || matches!(byte, b'[' | b']' | b'"' | b'#')
            {
                break;
            }
            self.position += 1;
        }

        str::from_utf8(&self.input[start..self.position])
            .map(Token::Word)
            .map_err(|_| LexError::NotUtf8 { offset: start })
    }

    /// A string literal, unescaped.
    ///
    /// Scans for the closing quote with `memchr` and only copies when an
    /// escape is actually present.
    fn string(&mut self) -> Result<Token<'a>, LexError> {
        let open = self.position;
        self.position += 1;
        let start = self.position;

        let mut escaped = false;
        loop {
            let Some(&byte) = self.input.get(self.position) else {
                return Err(LexError::UnterminatedString { offset: open });
            };
            match byte {
                b'\\' => {
                    escaped = true;
                    // Skip the escaped byte, so an escaped quote does
                    // not end the literal.
                    self.position += 2;
                }
                b'"' => break,
                _ => self.position += 1,
            }
        }

        let raw = &self.input[start..self.position];
        self.position += 1;

        if escaped {
            unescape(raw, start).map(|text| Token::Quoted(Quoted::Owned(text)))
        } else {
            str::from_utf8(raw)
                .map(|text| Token::Quoted(Quoted::Borrowed(text)))
                .map_err(|_| LexError::NotUtf8 { offset: start })
        }
    }
}

/// Decode the escapes 3Delight writes.
///
/// Measured, not assumed: the renderer writes `\"`, `\\`, `\t` and
/// `\n` by name, every other byte below `0x20` as **three-digit
/// octal** (`\001`, `\015`), and every byte at or above `0x7f`
/// **raw**. There is no `\xHH`; decoding that instead rejected `\001`
/// outright, which made any stream carrying a tab or a carriage return
/// in a string unreadable.
///
/// Measured, not assumed: the renderer writes `\"`, `\\`, `\t` and
/// `\n` by name, every other byte below `0x20` as **three-digit
/// octal** (`\001`, `\015`), and every byte at or above `0x7f`
/// **raw**. There is no `\xHH`; an earlier version of this function
/// decoded that and rejected `\001`, which made a stream containing a
/// tab-separated attribute or a Windows path unreadable.
fn unescape(raw: &[u8], offset: usize) -> Result<String, LexError> {
    let mut out = Vec::with_capacity(raw.len());
    let mut index = 0;

    while index < raw.len() {
        if raw[index] == b'\\' {
            let Some(&escape) = raw.get(index + 1) else {
                return Err(LexError::BadEscape {
                    offset: offset + index,
                });
            };
            match escape {
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'"' => out.push(b'"'),
                b'\\' => out.push(b'\\'),
                b'0'..=b'7' => {
                    // Three octal digits, as the renderer writes them.
                    let digits = raw.get(index + 1..index + 4).ok_or(
                        LexError::BadEscape {
                            offset: offset + index,
                        },
                    )?;
                    if !digits
                        .iter()
                        .all(|digit| digit.is_ascii_digit() && *digit < b'8')
                    {
                        return Err(LexError::BadEscape {
                            offset: offset + index,
                        });
                    }
                    let value = digits
                        .iter()
                        .fold(0u32, |acc, d| acc * 8 + u32::from(d - b'0'));
                    out.push(u8::try_from(value).map_err(|_| {
                        LexError::BadEscape {
                            offset: offset + index,
                        }
                    })?);
                    // Two beyond the one the shared step consumes.
                    index += 2;
                }
                _ => {
                    return Err(LexError::BadEscape {
                        offset: offset + index,
                    });
                }
            }
            index += 2;
        } else {
            out.push(raw[index]);
            index += 1;
        }
    }

    String::from_utf8(out).map_err(|_| LexError::NotUtf8 { offset })
}
