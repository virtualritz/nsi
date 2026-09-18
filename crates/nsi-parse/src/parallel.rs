//! Parsing and applying a stream on every core.
//!
//! Three steps. A sequential **scan** lexes the stream once to find where
//! each statement starts, what kind it is and which node or edge it
//! touches. The statements between two **barriers** then form a segment,
//! applied in two parallel **phases**: every `Create`; then every
//! `SetAttribute`, `SetAttributeAtTime` and `Connect`. Within a phase,
//! statements with the same key keep their stream order and statements
//! with different keys run concurrently. A barrier is applied alone,
//! between segments. See `specs/010-parallel-parse`.

use crate::{
    Error,
    lex::{Lexer, Token},
    parse::{apply, is_keyword},
    value::Scratch,
};
use core::hash::{BuildHasher, Hash, Hasher};
use nsi_trait::Nsi;
use rayon::prelude::*;

/// Where a statement goes in the schedule.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Phase {
    /// `Create`: before anything can refer to the node.
    Create,
    /// `SetAttribute`, `SetAttributeAtTime` and `Connect`: once every
    /// node exists, these touch one node or one edge each.
    Modify,
    /// Everything else: ordered against every statement around it.
    Barrier,
}

/// One statement, as the scan found it.
#[derive(Copy, Clone, Debug)]
struct Statement {
    phase: Phase,
    /// Offset of the keyword.
    start: usize,
    /// What the statement touches, hashed: the handle, or for `Connect`
    /// the destination node and attribute. Two statements with one key
    /// keep their order.
    ///
    /// **Not the whole edge.** Two connections of equal priority into
    /// one attribute are resolved by arrival: rendered, a surface
    /// connected through `attrA` then `attrB` is `attrA`'s shader, and
    /// the reverse order gives `attrB`'s. See D3 in the spec's
    /// `research.md`.
    ///
    /// **A collision only costs parallelism.** Two keys hashing alike
    /// put two groups into one, which runs them in stream order -- still
    /// correct.
    key: u64,
}

/// Which quoted operands name what a statement touches: how many to
/// skip, then how many make the key.
fn phase_and_operands(keyword: &str) -> (Phase, usize, usize) {
    match keyword {
        "Create" => (Phase::Create, 0, 1),
        "SetAttribute" | "SetAttributeAtTime" => (Phase::Modify, 0, 1),
        // `from`, `from_attribute`, then the key: `to`, `to_attribute`.
        "Connect" => (Phase::Modify, 2, 2),
        _ => (Phase::Barrier, 0, 0),
    }
}

/// Bytes that end a bare word, as the lexer has it: whitespace, a
/// bracket, a quote, or a comment's `#`.
const ENDS_WORD: [bool; 256] = {
    let mut table = [false; 256];
    let mut byte = 0;
    while byte < 256 {
        table[byte] = (byte as u8).is_ascii_whitespace()
            || matches!(byte as u8, b'[' | b']' | b'"' | b'#');
        byte += 1;
    }
    table
};

/// Finds every statement: its keyword's offset, its phase and its key.
///
/// The one sequential step, so it does the least that finds a boundary:
/// it does not build tokens, check UTF-8 or decode values, only skips
/// strings -- a string may hold anything, so a keyword is only a
/// keyword outside one -- and compares bare words against the keywords.
/// Whatever it skips is checked when its statement is parsed, by the
/// same lexer the sequential parser uses, so errors are unchanged.
///
/// `None` when the stream does not start with a keyword: the sequential
/// parser then fails at that first token, before touching the sink, and
/// reports exactly what it would.
fn scan(input: &[u8]) -> Option<Vec<Statement>> {
    let hasher = ahash::RandomState::new();
    let mut statements = Vec::new();
    // The statement being read, its key so far, and how many quoted
    // operands to skip and then take into the key.
    let mut current: Option<(Statement, ahash::AHasher, usize, usize)> = None;
    let mut position = 0;

    while let Some(&byte) = input.get(position) {
        match byte {
            _ if byte.is_ascii_whitespace() => position += 1,
            b'#' => {
                position = memchr::memchr(b'\n', &input[position..])
                    .map_or(input.len(), |end| position + end);
            }
            b'"' => {
                let open = position;
                // The closing quote, stepping over each escape's next
                // byte as the lexer does. Unterminated, the rest of the
                // input is this string, and the lexer says so later.
                let mut cursor = open + 1;
                let close = loop {
                    match memchr::memchr2(b'"', b'\\', &input[cursor..]) {
                        None => break input.len(),
                        Some(at) if input[cursor + at] == b'\\' => {
                            cursor += at + 2
                        }
                        Some(at) => break cursor + at,
                    }
                };
                position = close + 1;

                let (_, key, skip, take) = current.as_mut()?;
                if 0 < *skip {
                    *skip -= 1;
                } else if 0 < *take {
                    let raw = &input[(open + 1).min(close)..close];
                    // An escaped handle is keyed by what it spells, so
                    // two spellings of one handle stay in one group. An
                    // escape that does not decode is reported when the
                    // statement is parsed; any key will do until then.
                    match raw.contains(&b'\\') {
                        true => crate::lex::unescape(raw, open)
                            .unwrap_or_else(|_| raw.to_vec())
                            .hash(key),
                        false => raw.hash(key),
                    }
                    *take -= 1;
                }
            }
            b'[' | b']' => {
                // A value, which ends the key.
                current.as_mut()?.3 = 0;
                position += 1;
            }
            _ => {
                let start = position;
                while input
                    .get(position)
                    .is_some_and(|&b| !ENDS_WORD[b as usize])
                {
                    position += 1;
                }
                let word = &input[start..position];

                // Every keyword starts upper case and no number does, so
                // most words -- the values -- are rejected on one byte.
                let keyword = word
                    .first()
                    .is_some_and(u8::is_ascii_uppercase)
                    .then(|| core::str::from_utf8(word).ok())
                    .flatten()
                    .filter(|word| is_keyword(word));

                match keyword {
                    Some(keyword) => {
                        statements.extend(current.take().map(finish));
                        let (phase, skip, take) = phase_and_operands(keyword);
                        current = Some((
                            Statement {
                                phase,
                                start,
                                key: 0,
                            },
                            hasher.build_hasher(),
                            skip,
                            take,
                        ));
                    }
                    // An operand that is not a string ends the key: the
                    // `SetAttributeAtTime` time, say, or a value.
                    None => current.as_mut()?.3 = 0,
                }
            }
        }
    }

    statements.extend(current.map(finish));
    Some(statements)
}

/// A scanned statement with its key complete.
fn finish(
    (statement, key, _, _): (Statement, ahash::AHasher, usize, usize),
) -> Statement {
    Statement {
        key: key.finish(),
        ..statement
    }
}

/// Parses and applies the statement at `start`, which the scan says
/// ends at `end`.
///
/// Also judges whatever lies between the statement's last operand and
/// `end`: the sequential parser reads it as the next statement's
/// keyword, so anything else there is the same syntax error.
fn apply_one<'a, N>(
    input: &'a [u8],
    start: usize,
    end: usize,
    sink: &N,
    scratch: &mut Scratch<'a>,
) -> Result<(), Error<N::Error>>
where
    N: Nsi,
    for<'call> N: Nsi<Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>>,
{
    let mut lexer = Lexer::starting_at(input, start);
    // SAFETY: `scan` records a statement only at a keyword token, and
    // re-lexing from its offset reads the same token.
    let Some(Token::Word(keyword)) = lexer.next_token()? else {
        unreachable!("the scan starts every statement at its keyword")
    };

    let next = match apply(keyword, &mut lexer, sink, scratch)? {
        Some(token) => Some(token),
        None => lexer.next_token()?,
    };
    match next {
        None => Ok(()),
        Some(Token::Word(word)) if is_keyword(word) => {
            debug_assert_eq!(lexer.offset(), end);
            Ok(())
        }
        Some(_) => Err(Error::Syntax {
            offset: lexer.offset(),
            expected: "a statement keyword",
        }),
    }
}

/// Applies `indices` of `statements`, grouped by key: groups in
/// parallel, each group in stream order.
///
/// On failure, the error of the earliest failing statement. Every group
/// stops at its own first failure; the others run to the end.
fn run_phase<N>(
    input: &[u8],
    statements: &[Statement],
    mut indices: Vec<usize>,
    sink: &N,
) -> Result<(), Error<N::Error>>
where
    N: Nsi + Sync,
    for<'call> N: Nsi<Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>>,
{
    // Stable, so a group is in stream order.
    indices.sort_by_key(|&index| statements[index].key);

    let end_of = |index: usize| {
        statements
            .get(index + 1)
            .map_or(input.len(), |next| next.start)
    };

    indices
        .par_chunk_by(|&a, &b| statements[a].key == statements[b].key)
        .map_init(Scratch::default, |scratch, group| {
            group.iter().find_map(|&index| {
                let start = statements[index].start;
                apply_one(input, start, end_of(index), sink, scratch)
                    .err()
                    .map(|error| (start, error))
            })
        })
        .flatten()
        .min_by_key(|(start, _)| *start)
        .map_or(Ok(()), |(_, error)| Err(error))
}

/// Parse `input` and apply it to `sink`, on every core.
pub(crate) fn parse<N>(input: &[u8], sink: &N) -> Result<(), Error<N::Error>>
where
    N: Nsi + Sync,
    for<'call> N: Nsi<Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>>,
{
    let Some(statements) = scan(input) else {
        return crate::parse::parse(input, sink);
    };

    let mut segment_start = 0;
    for (index, statement) in statements.iter().enumerate() {
        let is_last = index + 1 == statements.len();
        if statement.phase != Phase::Barrier && !is_last {
            continue;
        }

        // The segment runs up to this barrier, or through the last
        // statement when that is not one.
        let segment_end = if statement.phase == Phase::Barrier {
            index
        } else {
            index + 1
        };
        let phase = |wanted: Phase| {
            (segment_start..segment_end)
                .filter(|&i| statements[i].phase == wanted)
                .collect::<Vec<_>>()
        };
        run_phase(input, &statements, phase(Phase::Create), sink)?;
        run_phase(input, &statements, phase(Phase::Modify), sink)?;

        if statement.phase == Phase::Barrier {
            let end = statements
                .get(index + 1)
                .map_or(input.len(), |next| next.start);
            apply_one(
                input,
                statement.start,
                end,
                sink,
                &mut Scratch::default(),
            )?;
        }
        segment_start = index + 1;
    }

    Ok(())
}
