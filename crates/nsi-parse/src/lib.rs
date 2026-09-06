//! A fast reader for ɴsɪ scenes.
//!
//! `nsi-intermediate` writes ɴsɪ out; this reads it back. Together they
//! close the loop: a scene can be captured, inspected, replayed and
//! round-tripped without a renderer.
//!
//! # It calls you
//!
//! Parsing drives [`nsi_trait::Nsi`] rather than producing a scene type
//! of its own, so the same parser feeds a live 3Delight context, an
//! `nsi-intermediate` `Recorder`, or a backend's own implementation. A
//! reader that insisted on its own representation would make every
//! consumer translate.
//!
//! A sink brings its own *behaviour*, not its own argument type: every
//! entry point here binds `Nsi<Arg<'call> = nsi_ffi_wrap::Arg<'call,
//! 'static>>`, because the parser has to build the arguments it hands
//! over and can only build the one shape. Implement [`nsi_trait::Nsi`]
//! with that associated type and the parser will drive it.
//!
//! ```no_run
//! # use nsi_parse::parse_stream;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let bytes = std::fs::read("scene.nsi")?;
//! let recorder = nsi_intermediate::Recorder::new();
//! parse_stream(&bytes, &recorder)?;
//! # Ok(())
//! # }
//! ```
//!
//! # The grammar is observed, not specified
//!
//! ɴsɪ publishes no grammar for its stream, only examples. This parser
//! is written against what 3Delight accepts, and the decisive
//! observation is that **an entire scene on one line parses**: the
//! newlines and indents a renderer writes are formatting, not syntax.
//! So a parameter list runs until the next *bare* token naming a
//! statement -- parameter names are always quoted, which makes that
//! unambiguous -- and a line-oriented reader would be wrong on valid
//! input.
//!
//! # Features
//!
//! | Feature | What it adds |
//! | --- | --- |
//! | *(none)* | The `.nsi` stream reader. |
//! | `lua` | Reading a Lua scene, which **runs** the script. Builds Lua 5.4 from vendored C source. |
//! | `gzip` | Reading a gzip-compressed stream. |
//! | `zstd` | Reading a zstd-compressed stream. |
//!
//! Reading a Lua scene means executing it. ɴsɪ's Lua front end is a
//! programming language -- a script may compute the scene it describes
//! -- so an interpreter is the only correct reader, and that is a
//! different trust decision from parsing a data file.

#![deny(missing_docs)]

mod lex;
#[cfg(feature = "lua")]
mod lua;
mod parse;
mod value;

use core::fmt;
use nsi_trait::Nsi;
#[cfg(feature = "gzip")]
use std::io::Read;

/// Why a scene could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error<E> {
    /// The input is not the grammar.
    Syntax {
        /// Byte offset into the input.
        offset: usize,
        /// What the parser wanted there.
        expected: &'static str,
    },
    /// A string literal ran to end of input.
    UnterminatedString {
        /// Byte offset of the opening quote.
        offset: usize,
    },
    /// A byte sequence that is not UTF-8.
    NotUtf8 {
        /// Byte offset.
        offset: usize,
    },
    /// The input is a `binarynsi` stream, which this crate cannot read.
    ///
    /// ɴsɪ has three stream formats and this reads one of them. The
    /// binary encoding is undocumented, and `autonsi` -- 3Delight's
    /// default for a name not ending `.nsia` -- selects it, so a caller
    /// can be handed one without having chosen it.
    ///
    /// Detected rather than left to fail as "not UTF-8 at byte 0",
    /// which is true and useless: it sends the reader looking for an
    /// encoding problem instead of a format one.
    BinaryStream,
    /// An escape sequence this format does not define.
    BadEscape {
        /// Byte offset of the backslash.
        offset: usize,
    },
    /// A Lua script failed to load or run.
    #[cfg(feature = "lua")]
    Lua(String),
    /// The input announced a compressor and would not decompress.
    #[cfg(any(feature = "gzip", feature = "zstd"))]
    Decompress(String),
    /// The sink refused a statement.
    ///
    /// Parsing stops here, and the sink keeps everything applied before
    /// it. A caller that wants all-or-nothing parses into a `Recorder`
    /// first and commits afterwards.
    Sink(E),
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { offset, expected } => {
                write!(f, "at byte {offset}: expected {expected}")
            }
            Self::UnterminatedString { offset } => {
                write!(f, "at byte {offset}: unterminated string")
            }
            Self::BinaryStream => write!(
                f,
                "this is a binarynsi stream, which this crate does not \
                 read; re-write it as text with `renderdl -cat`"
            ),
            Self::NotUtf8 { offset } => {
                write!(f, "at byte {offset}: not UTF-8")
            }
            Self::BadEscape { offset } => {
                write!(f, "at byte {offset}: unknown escape")
            }
            #[cfg(feature = "lua")]
            Self::Lua(message) => write!(f, "lua: {message}"),
            #[cfg(any(feature = "gzip", feature = "zstd"))]
            Self::Decompress(message) => {
                write!(f, "could not decompress: {message}")
            }
            Self::Sink(error) => write!(f, "the sink refused: {error}"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> core::error::Error for Error<E> {}

impl<E> From<lex::LexError> for Error<E> {
    fn from(error: lex::LexError) -> Self {
        match error {
            lex::LexError::UnterminatedString { offset } => {
                Self::UnterminatedString { offset }
            }
            lex::LexError::NotUtf8 { offset } => Self::NotUtf8 { offset },
            lex::LexError::BadEscape { offset } => Self::BadEscape { offset },
        }
    }
}

#[cfg(feature = "lua")]
pub use lua::run_lua;

/// Read an ɴsɪ stream, applying it to `sink`.
///
/// # Errors
///
/// [`Error::Syntax`] and its neighbours for input that is not the
/// grammar, and [`Error::Sink`] when the sink refuses a statement.
pub fn parse_stream<N>(input: &[u8], sink: &N) -> Result<(), Error<N::Error>>
where
    N: Nsi,
    for<'call> N: Nsi<Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>>,
{
    // `0xCC` is a UTF-8 continuation byte, so a text stream can never
    // begin with it. Measured: every `renderdl -cat -binary` output
    // starts `cc 00`, whatever statement comes first -- the third byte
    // is a length tag for the keyword that follows.
    if input.starts_with(&[0xCC, 0x00]) {
        return Err(Error::BinaryStream);
    }

    parse::parse(input, sink)
}

/// Read an ɴsɪ stream that may be compressed, applying it to `sink`.
///
/// The compressor is detected from the input's leading bytes, so a
/// caller that took a file from a pipeline does not have to know how it
/// was written. Uncompressed input is passed straight through.
///
/// Only gzip is a format 3Delight itself reads; zstd is supported here
/// for consumers of this workspace. See `nsi-intermediate`'s
/// `Compression`.
///
/// # Errors
///
/// [`Error::Decompress`] when the input announces a compressor and then
/// fails to decompress, plus everything [`parse_stream`] returns.
#[cfg(any(feature = "gzip", feature = "zstd"))]
pub fn parse_compressed<N>(
    input: &[u8],
    sink: &N,
) -> Result<(), Error<N::Error>>
where
    N: Nsi,
    for<'call> N: Nsi<Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>>,
{
    #[cfg(feature = "gzip")]
    if input.starts_with(&[0x1f, 0x8b]) {
        let mut plain = Vec::new();
        flate2::read::GzDecoder::new(input)
            .read_to_end(&mut plain)
            .map_err(|error| Error::Decompress(error.to_string()))?;
        return parse_stream(&plain, sink);
    }

    #[cfg(feature = "zstd")]
    if input.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        let plain = zstd::decode_all(input)
            .map_err(|error| Error::Decompress(error.to_string()))?;
        return parse_stream(&plain, sink);
    }

    parse_stream(input, sink)
}
