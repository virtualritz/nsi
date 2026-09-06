//! Statement parsing, and application to a sink.
//!
//! The grammar is keyword-terminated: a parameter list runs until the
//! next *bare* token that names a statement. Parameter names are always
//! quoted, so a bare word is unambiguous, and that one rule is what lets
//! a statement occupy one line or twenty. See `specs/004-nsi-parse` D2.

use crate::{
    Error,
    lex::{Ident, Lexer, Token},
    value::{self, Scratch},
};
use nsi_trait::Nsi;

/// Every statement 3Delight writes.
const KEYWORDS: [&str; 9] = [
    "Create",
    "SetAttribute",
    "SetAttributeAtTime",
    "Delete",
    "DeleteAttribute",
    "Connect",
    "Disconnect",
    "Evaluate",
    "RenderControl",
];

fn is_keyword(word: &str) -> bool {
    KEYWORDS.contains(&word)
}

/// Parse `input` and apply it to `sink`.
pub(crate) fn parse<N>(input: &[u8], sink: &N) -> Result<(), Error<N::Error>>
where
    N: Nsi,
    for<'call> N: Nsi<Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>>,
{
    let mut lexer = Lexer::new(input);
    let mut pending = lexer.next_token()?;
    let mut scratch = Scratch::default();

    while let Some(token) = pending.take() {
        let offset = lexer.offset();
        let Token::Word(keyword) = token else {
            return Err(Error::Syntax {
                offset,
                expected: "a statement keyword",
            });
        };
        if !is_keyword(keyword) {
            return Err(Error::Syntax {
                offset,
                expected: "a statement keyword",
            });
        }

        pending = apply(keyword, &mut lexer, sink, &mut scratch)?;
        if pending.is_none() {
            pending = lexer.next_token()?;
        }
    }

    Ok(())
}

/// Read one statement's operands, apply it, and return the token that
/// ended it if one was read ahead.
fn apply<'a, N>(
    keyword: &str,
    lexer: &mut Lexer<'a>,
    sink: &N,
    scratch: &mut Scratch<'a>,
) -> Result<Option<Token<'a>>, Error<N::Error>>
where
    N: Nsi,
    for<'call> N: Nsi<Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>>,
{
    match keyword {
        "Create" => {
            let handle = string(lexer, "a node handle")?;
            let node_type = string(lexer, "a node type")?;
            // ɴsɪ's `NSICreate` takes optional parameters and 3Delight
            // writes them. Stopping here rejected legal renderer output.
            let next = parameters(lexer, scratch)?;
            scratch
                .with_args(|args| {
                    sink.create(handle.as_str(), node_type.as_str(), Some(args))
                })
                .map_err(Error::Sink)?;
            Ok(next)
        }
        "Delete" => {
            let handle = string(lexer, "a node handle")?;
            let next = parameters(lexer, scratch)?;
            scratch
                .with_args(|args| sink.delete(handle.as_str(), Some(args)))
                .map_err(Error::Sink)?;
            Ok(next)
        }
        "DeleteAttribute" => {
            let handle = string(lexer, "a node handle")?;
            let name = string(lexer, "an attribute name")?;
            sink.delete_attribute(handle.as_str(), name.as_str())
                .map_err(Error::Sink)?;
            Ok(None)
        }
        "SetAttribute" => {
            let handle = string(lexer, "a node handle")?;
            let next = parameters(lexer, scratch)?;
            scratch
                .with_args(|args| sink.set_attribute(handle.as_str(), args))
                .map_err(Error::Sink)?;
            Ok(next)
        }
        "SetAttributeAtTime" => {
            let handle = string(lexer, "a node handle")?;
            let time = number(lexer)?;
            let next = parameters(lexer, scratch)?;
            scratch
                .with_args(|args| {
                    sink.set_attribute_at_time(handle.as_str(), time, args)
                })
                .map_err(Error::Sink)?;
            Ok(next)
        }
        "Connect" | "Disconnect" => {
            let from = string(lexer, "a source handle")?;
            let from_attr = string(lexer, "a source attribute")?;
            let to = string(lexer, "a destination handle")?;
            let to_attr = string(lexer, "a destination attribute")?;

            // ɴsɪ writes an unnamed source port as the empty string, and
            // documents that as equivalent to none.
            let port = Some(from_attr.as_str()).filter(|p| !p.is_empty());

            if keyword == "Connect" {
                let next = parameters(lexer, scratch)?;
                scratch
                    .with_args(|args| {
                        sink.connect(
                            from.as_str(),
                            port,
                            to.as_str(),
                            to_attr.as_str(),
                            Some(args),
                        )
                    })
                    .map_err(Error::Sink)?;
                Ok(next)
            } else {
                sink.disconnect(
                    from.as_str(),
                    port,
                    to.as_str(),
                    to_attr.as_str(),
                )
                .map_err(Error::Sink)?;
                Ok(None)
            }
        }
        "Evaluate" => {
            let next = parameters(lexer, scratch)?;
            scratch
                .with_args(|args| sink.evaluate(args))
                .map_err(Error::Sink)?;
            Ok(next)
        }
        "RenderControl" => {
            let next = parameters(lexer, scratch)?;
            // ɴsɪ gives no default, and guessing `Start` would begin a
            // render from a statement that did not ask for one.
            let action = scratch.action().ok_or(Error::Syntax {
                offset: lexer.offset(),
                expected: "a RenderControl action",
            })?;
            // ɴsɪ's own `render_control` appends the action, so passing
            // it through here would write it twice.
            scratch
                .with_args_except(Some("action"), |args| {
                    sink.render_control(action, Some(args))
                })
                .map_err(Error::Sink)?;
            Ok(next)
        }
        // `parse` checked membership before dispatching.
        _ => unreachable!("dispatched on a keyword"),
    }
}

/// One quoted operand that names something.
///
/// Every caller is a handle, node type, attribute name or type
/// spelling, so the UTF-8 check belongs here rather than at each use.
/// A string *value* is read by `value::read` and stays bytes.
fn string<'a, E>(
    lexer: &mut Lexer<'a>,
    expected: &'static str,
) -> Result<Ident<'a>, Error<E>> {
    match lexer.next_token()? {
        // After `next_token`, not before: `offset` reports the start of
        // the token last returned, so reading it first named the
        // *previous* operand.
        Some(Token::Quoted(text)) => Ok(text.into_ident(lexer.offset())?),
        _ => Err(Error::Syntax {
            offset: lexer.offset(),
            expected,
        }),
    }
}

/// One bare numeric operand.
fn number<E>(lexer: &mut Lexer<'_>) -> Result<f64, Error<E>> {
    match lexer.next_token()? {
        Some(Token::Word(text)) => text.parse().map_err(|_| Error::Syntax {
            offset: lexer.offset(),
            expected: "a number",
        }),
        _ => Err(Error::Syntax {
            offset: lexer.offset(),
            expected: "a number",
        }),
    }
}

/// Read a statement's parameter list into `scratch`.
///
/// Returns the token that ended it, which is the next statement's
/// keyword, so the caller does not re-read it.
fn parameters<'a, E>(
    lexer: &mut Lexer<'a>,
    scratch: &mut Scratch<'a>,
) -> Result<Option<Token<'a>>, Error<E>> {
    scratch.clear();

    loop {
        match lexer.next_token()? {
            // A quoted token here is a parameter name: parameter names
            // are always quoted and keywords never are.
            Some(Token::Quoted(name)) => {
                let name = name.into_ident(lexer.offset())?.into_cow();
                value::read(name, lexer, scratch)?;
            }
            other => return Ok(other),
        }
    }
}
