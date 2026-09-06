//! Parameter values, and the scratch they are parsed into.
//!
//! A statement's parameters are read in two passes: everything lands in
//! per-type buffers first, then the [`nsi_ffi_wrap::Arg`]s are built
//! over disjoint ranges of those buffers. One pass would not borrow-check
//! -- an `Arg` holds a slice while the next parameter still wants to
//! push -- and it is also what keeps the steady state allocation-free,
//! since the buffers are cleared rather than freed.

use crate::{
    Error,
    lex::{Lexer, Token},
};
use alloc_free::{SmallArgs, SmallStrs};
use core::num::NonZeroUsize;
use nsi_ffi_wrap::{
    Arg, ArgData, ColorSlice, F32Slice, F64Slice, I32Slice, I64Slice,
    MatrixF32Slice, MatrixF64Slice, NormalSlice, PointSlice, StringSlice,
    VectorSlice,
};
use nsi_trait::Action;
use std::borrow::Cow;

/// Which buffer a parameter's values live in, and how to rebuild it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    F32,
    F64,
    I32,
    I64,
    String,
    Color,
    Point,
    Vector,
    Normal,
    MatrixF32,
    MatrixF64,
}

impl Base {
    /// The ɴsɪ stream spelling, without flags or array suffix.
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "float" => Self::F32,
            "double" => Self::F64,
            "int" => Self::I32,
            "int64" => Self::I64,
            "string" => Self::String,
            "color" => Self::Color,
            "point" => Self::Point,
            "vector" => Self::Vector,
            "normal" => Self::Normal,
            "matrix" => Self::MatrixF32,
            "doublematrix" => Self::MatrixF64,
            _ => return None,
        })
    }

    /// Whether values go in the float, double, integer or string buffer.
    const fn storage(self) -> Storage {
        match self {
            Self::F32
            | Self::Color
            | Self::Point
            | Self::Vector
            | Self::Normal
            | Self::MatrixF32 => Storage::F32,
            Self::F64 | Self::MatrixF64 => Storage::F64,
            Self::I32 => Storage::I32,
            Self::I64 => Storage::I64,
            Self::String => Storage::String,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Storage {
    F32,
    F64,
    I32,
    I64,
    String,
}

/// One parsed parameter, as a range into the scratch buffers.
struct Descriptor<'a> {
    name: Cow<'a, str>,
    base: Base,
    /// ɴsɪ's `array_len`, when the type carried a `[n]` suffix.
    array_length: Option<NonZeroUsize>,
    /// `v`, `f` and `l` prefixes.
    per_vertex: bool,
    per_face: bool,
    linear: bool,
    start: usize,
    end: usize,
}

/// Reused buffers for one statement's parameters.
#[derive(Default)]
pub(crate) struct Scratch<'a> {
    descriptors: Vec<Descriptor<'a>>,
    f32s: Vec<f32>,
    f64s: Vec<f64>,
    i32s: Vec<i32>,
    i64s: Vec<i64>,
    /// Borrowed from the input unless the literal carried escapes.
    strings: Vec<Cow<'a, str>>,
    /// Colour, point, vector and normal, which the argument types take
    /// as triples rather than as a flat slice.
    triples: Vec<[f32; 3]>,
    matrices_f32: Vec<[f32; 16]>,
    matrices_f64: Vec<[f64; 16]>,
}

impl<'a> Scratch<'a> {
    /// Drop the previous statement's parameters, keeping the capacity.
    pub(crate) fn clear(&mut self) {
        self.descriptors.clear();
        self.f32s.clear();
        self.f64s.clear();
        self.i32s.clear();
        self.i64s.clear();
        self.strings.clear();
        self.triples.clear();
        self.matrices_f32.clear();
        self.matrices_f64.clear();
    }

    /// ɴsɪ's `RenderControl` action, if the parameters name one.
    pub(crate) fn action(&self) -> Option<Action> {
        let descriptor = self
            .descriptors
            .iter()
            .find(|d| d.name == "action" && d.base == Base::String)?;
        match self.strings.get(descriptor.start)?.as_ref() {
            "start" => Some(Action::Start),
            "stop" => Some(Action::Stop),
            "suspend" => Some(Action::Suspend),
            "resume" => Some(Action::Resume),
            "wait" => Some(Action::Wait),
            "synchronize" => Some(Action::Synchronize),
            _ => None,
        }
    }

    /// Build the arguments and hand them to `apply`.
    ///
    /// They borrow the scratch, so they cannot outlive the call -- which
    /// is exactly the lifetime `Nsi`'s methods want.
    pub(crate) fn with_args<T, E>(
        &self,
        apply: impl FnOnce(&[Arg<'_, 'static>]) -> Result<T, E>,
    ) -> Result<T, E> {
        self.with_args_except(None, apply)
    }

    /// The same, without the parameter of the given name.
    ///
    /// `RenderControl` needs it: the action is passed as an argument to
    /// `Nsi::render_control`, which appends it again.
    pub(crate) fn with_args_except<T, E>(
        &self,
        skip: Option<&str>,
        apply: impl FnOnce(&[Arg<'_, 'static>]) -> Result<T, E>,
    ) -> Result<T, E> {
        // `StringSlice` wants `&[&str]`. The view is on the stack:
        // heap-allocating it would put an allocation back on every
        // statement that carries a string.
        let borrowed: SmallStrs<'_> =
            self.strings.iter().map(Cow::as_ref).collect();

        let args: SmallArgs<'_> = self
            .descriptors
            .iter()
            .filter(|d| Some(d.name.as_ref()) != skip)
            .map(|d| {
                let data = match d.base {
                    Base::F32 => {
                        ArgData::from(F32Slice::new(&self.f32s[d.start..d.end]))
                    }
                    Base::F64 => {
                        ArgData::from(F64Slice::new(&self.f64s[d.start..d.end]))
                    }
                    Base::I32 => {
                        ArgData::from(I32Slice::new(&self.i32s[d.start..d.end]))
                    }
                    Base::I64 => {
                        ArgData::from(I64Slice::new(&self.i64s[d.start..d.end]))
                    }
                    Base::String => ArgData::from(StringSlice::new(
                        &borrowed[d.start..d.end],
                    )),
                    Base::Color => ArgData::from(ColorSlice::new(
                        &self.triples[d.start..d.end],
                    )),
                    Base::Point => ArgData::from(PointSlice::new(
                        &self.triples[d.start..d.end],
                    )),
                    Base::Vector => ArgData::from(VectorSlice::new(
                        &self.triples[d.start..d.end],
                    )),
                    Base::Normal => ArgData::from(NormalSlice::new(
                        &self.triples[d.start..d.end],
                    )),
                    Base::MatrixF32 => ArgData::from(MatrixF32Slice::new(
                        &self.matrices_f32[d.start..d.end],
                    )),
                    Base::MatrixF64 => ArgData::from(MatrixF64Slice::new(
                        &self.matrices_f64[d.start..d.end],
                    )),
                };

                let mut arg = Arg::new(&d.name, data);
                if let Some(length) = d.array_length {
                    arg = arg.array_len(length);
                }
                if d.per_vertex {
                    arg = arg.per_vertex();
                }
                if d.per_face {
                    arg = arg.per_face();
                }
                if d.linear {
                    arg = arg.linear_interpolation();
                }
                arg
            })
            .collect();

        apply(&args)
    }
}

/// Read one parameter -- `"name" "type" count value...` -- into scratch.
pub(crate) fn read<'a, E>(
    name: Cow<'a, str>,
    lexer: &mut Lexer<'a>,
    scratch: &mut Scratch<'a>,
) -> Result<(), Error<E>> {
    let Some(Token::Quoted(spelling)) = lexer.next_token()? else {
        return Err(Error::Syntax {
            offset: lexer.offset(),
            expected: "a parameter type",
        });
    };
    let type_offset = lexer.offset();

    let (base, array_length, per_vertex, per_face, linear) =
        parse_type(spelling.as_str()).ok_or(Error::Syntax {
            offset: type_offset,
            expected: "a known ɴsɪ type",
        })?;

    // The count is authoritative, not decorative: given
    // `"P" "point" 1 [ 0 0 0 1 2 3 ]` the renderer warns and keeps one
    // point. Ignoring it here would silently yield two.
    let count: usize = match lexer.next_token()? {
        Some(Token::Word(text)) => text.parse().map_err(|_| Error::Syntax {
            offset: lexer.offset(),
            expected: "an element count",
        })?,
        _ => {
            return Err(Error::Syntax {
                offset: lexer.offset(),
                expected: "an element count",
            });
        }
    };
    let count_offset = lexer.offset();

    // Where this parameter's flat values begin. Folding must move only
    // its own run: an earlier `float` parameter in the same statement
    // still owns the values before it.
    let flat = (scratch.f32s.len(), scratch.f64s.len());

    let start = start_of(scratch, base);
    read_values(lexer, scratch, base)?;
    fold(scratch, base, flat).map_err(|()| Error::Syntax {
        offset: type_offset,
        expected: "a whole number of elements for the type",
    })?;
    let end = start_of(scratch, base);

    // Elements, times the array length the type spelling carried.
    let declared = count
        .checked_mul(array_length.map_or(1, NonZeroUsize::get))
        .ok_or(Error::Syntax {
            offset: count_offset,
            expected: "an element count that does not overflow",
        })?;
    if end - start != declared {
        return Err(Error::Syntax {
            offset: count_offset,
            expected: "as many values as the count declares",
        });
    }

    scratch.descriptors.push(Descriptor {
        name,
        base,
        array_length,
        per_vertex,
        per_face,
        linear,
        start,
        end,
    });

    Ok(())
}

/// The current length of the buffer `base` is *indexed by*.
///
/// For a tuple type that is the folded buffer, not the flat one the
/// values are read into.
fn start_of(scratch: &Scratch<'_>, base: Base) -> usize {
    match base {
        Base::Color | Base::Point | Base::Vector | Base::Normal => {
            scratch.triples.len()
        }
        Base::MatrixF32 => scratch.matrices_f32.len(),
        Base::MatrixF64 => scratch.matrices_f64.len(),
        Base::F32 => scratch.f32s.len(),
        Base::F64 => scratch.f64s.len(),
        Base::I32 => scratch.i32s.len(),
        Base::I64 => scratch.i64s.len(),
        Base::String => scratch.strings.len(),
    }
}

/// Move a tuple type's flat values into its typed buffer.
///
/// The argument types take `&[[f32; 3]]` and `&[[f32; 16]]`, so the
/// flat run a stream writes has to be grouped. A run that is not a whole
/// number of elements is malformed input, not something to truncate.
fn fold(
    scratch: &mut Scratch<'_>,
    base: Base,
    (flat_f32, flat_f64): (usize, usize),
) -> Result<(), ()> {
    match base {
        Base::Color | Base::Point | Base::Vector | Base::Normal => {
            let run = &scratch.f32s[flat_f32..];
            if !run.len().is_multiple_of(3) {
                return Err(());
            }
            for chunk in run.as_chunks::<3>().0 {
                scratch.triples.push([chunk[0], chunk[1], chunk[2]]);
            }
            scratch.f32s.truncate(flat_f32);
        }
        Base::MatrixF32 => {
            let run = &scratch.f32s[flat_f32..];
            if !run.len().is_multiple_of(16) {
                return Err(());
            }
            for chunk in run.as_chunks::<16>().0 {
                scratch.matrices_f32.push(*chunk);
            }
            scratch.f32s.truncate(flat_f32);
        }
        Base::MatrixF64 => {
            let run = &scratch.f64s[flat_f64..];
            if !run.len().is_multiple_of(16) {
                return Err(());
            }
            for chunk in run.as_chunks::<16>().0 {
                scratch.matrices_f64.push(*chunk);
            }
            scratch.f64s.truncate(flat_f64);
        }
        _ => {}
    }
    Ok(())
}

/// Read a bare scalar or a bracketed list into the right buffer.
fn read_values<'a, E>(
    lexer: &mut Lexer<'a>,
    scratch: &mut Scratch<'a>,
    base: Base,
) -> Result<(), Error<E>> {
    match lexer.next_token()? {
        Some(Token::Open) => loop {
            match lexer.next_token()? {
                Some(Token::Close) => return Ok(()),
                Some(token) => {
                    let offset = lexer.offset();
                    push(token, scratch, base, offset)?;
                }
                None => {
                    return Err(Error::Syntax {
                        offset: lexer.offset(),
                        expected: "a closing bracket",
                    });
                }
            }
        },
        Some(token) => {
            let offset = lexer.offset();
            push(token, scratch, base, offset)
        }
        None => Err(Error::Syntax {
            offset: lexer.offset(),
            expected: "a parameter value",
        }),
    }
}

/// Push one value, parsed as the parameter's storage type.
fn push<'a, E>(
    token: Token<'a>,
    scratch: &mut Scratch<'a>,
    base: Base,
    offset: usize,
) -> Result<(), Error<E>> {
    let bad = Error::Syntax {
        offset,
        expected: "a value of the parameter's type",
    };

    match (base.storage(), token) {
        (Storage::String, Token::Quoted(text)) => {
            // ɴsɪ strings become C strings, so an interior NUL would
            // panic in `CString::new`. 3Delight truncates there; this
            // refuses, because silently dropping half a string is the
            // kind of quiet wrong answer the crate exists to avoid.
            let text = text.into_cow();
            if text.contains('\0') {
                return Err(Error::Syntax {
                    offset,
                    expected: "a string without an interior NUL",
                });
            }
            scratch.strings.push(text);
            Ok(())
        }
        (Storage::F32, Token::Word(text)) => {
            scratch.f32s.push(text.parse().map_err(|_| bad)?);
            Ok(())
        }
        (Storage::F64, Token::Word(text)) => {
            scratch.f64s.push(text.parse().map_err(|_| bad)?);
            Ok(())
        }
        (Storage::I32, Token::Word(text)) => {
            scratch.i32s.push(text.parse().map_err(|_| bad)?);
            Ok(())
        }
        (Storage::I64, Token::Word(text)) => {
            scratch.i64s.push(text.parse().map_err(|_| bad)?);
            Ok(())
        }
        _ => Err(bad),
    }
}

/// Split a stream type spelling into its parts.
///
/// `"v point"` is a per-vertex point; `"int[2]"` is a two-element array
/// of integers; `"fl normal"` carries two flags. The flag letters come
/// first, separated by a space, and the array length rides inside the
/// type name -- both as `specs/003` recorded them from the renderer.
fn parse_type(
    spelling: &str,
) -> Option<(Base, Option<NonZeroUsize>, bool, bool, bool)> {
    let (flags, rest) = match spelling.split_once(' ') {
        Some((flags, rest)) => (flags, rest),
        None => ("", spelling),
    };

    let mut per_vertex = false;
    let mut per_face = false;
    let mut linear = false;
    for letter in flags.chars() {
        match letter {
            'v' => per_vertex = true,
            'f' => per_face = true,
            'l' => linear = true,
            _ => return None,
        }
    }

    let (name, array_length) = match rest.split_once('[') {
        Some((name, length)) => {
            let length = length.strip_suffix(']')?.parse().ok()?;
            (name, Some(NonZeroUsize::new(length)?))
        }
        None => (rest, None),
    };

    Some((
        Base::parse(name)?,
        array_length,
        per_vertex,
        per_face,
        linear,
    ))
}

/// A stack-backed argument list.
///
/// A statement's arguments cannot live in [`Scratch`] -- they borrow it
/// -- so without this every statement would allocate one vector. Eight
/// covers the parameter counts a renderer writes; more spills to the
/// heap rather than failing.
mod alloc_free {
    use nsi_ffi_wrap::Arg;

    pub(super) type SmallArgs<'a> = smallvec::SmallVec<[Arg<'a, 'static>; 8]>;
    pub(super) type SmallStrs<'a> = smallvec::SmallVec<[&'a str; 8]>;
}
