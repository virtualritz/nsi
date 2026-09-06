//! Reading a Lua scene, by running it.
//!
//! ɴsɪ's Lua front end is a programming language: a script may loop,
//! branch and compute the scene it describes, and the specification's
//! own examples do. So an interpreter is the only correct reader, and
//! pattern-matching `nsi.Create(...)` out of the source would work only
//! on scripts a generator wrote.
//!
//! **Reading a script executes it.** That is a different trust decision
//! from parsing a data file, which is why this is behind a feature and
//! said plainly rather than implied.

use crate::Error;
use core::num::NonZeroUsize;
use mlua::{Lua, Table, Value, Variadic};
use nsi_ffi_wrap::{
    Arg, ArgData, ColorSlice, F32Slice, I32Slice, MatrixF32Slice,
    MatrixF64Slice, NormalSlice, PointSlice, StringSlice, VectorSlice,
};
use nsi_trait::{Action, Nsi, Type};
use std::cell::RefCell;

/// One parameter, read out of a Lua table.
#[derive(Clone)]
struct Param {
    name: String,
    type_tag: Type,
    array_length: Option<NonZeroUsize>,
    f32s: Vec<f32>,
    f64s: Vec<f64>,
    i32s: Vec<i32>,
    i64s: Vec<i64>,
    strings: Vec<String>,
}

/// Run `source`, applying the ɴsɪ calls it makes to `sink`.
///
/// # Errors
///
/// [`Error::Lua`] for a script that fails to load or run, and
/// [`Error::Sink`] when the sink refuses a call.
pub fn run_lua<N>(source: &str, sink: &N) -> Result<(), Error<N::Error>>
where
    N: Nsi,
    for<'call> N: Nsi<Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>>,
{
    let lua = Lua::new();
    // A sink error is not a Lua error, so it is carried out of the
    // script rather than stringified into one.
    let failure: RefCell<Option<N::Error>> = RefCell::new(None);

    let outcome = lua.scope(|scope| {
        let nsi = lua.create_table()?;

        // Exactly the constants 3Delight's own `nsi` table has.
        // `TypeDouble` and `TypeInt64` are `nil` there, so offering them
        // here would let a script be written that the renderer refuses.
        for (name, tag) in [
            ("TypeFloat", Type::F32),
            ("TypeInteger", Type::I32),
            ("TypeString", Type::String),
            ("TypeColor", Type::Color),
            ("TypePoint", Type::Point),
            ("TypeVector", Type::Vector),
            ("TypeNormal", Type::Normal),
            ("TypeMatrix", Type::MatrixF32),
            ("TypeDoubleMatrix", Type::MatrixF64),
        ] {
            nsi.set(name, tag as i32)?;
        }

        macro_rules! record {
            ($body:expr) => {
                scope.create_function_mut($body)?
            };
        }

        nsi.set(
            "Create",
            record!(|_, (handle, node_type): (String, String)| {
                keep(&failure, sink.create(&handle, &node_type, None))
            }),
        )?;
        nsi.set(
            "Delete",
            record!(|_, (handle, rest): (String, Variadic<Value>)| {
                // ɴsɪ's `recursive` rides here; dropping it turned a
                // recursive delete into a plain one.
                let params = params_of(rest)?;
                with_args(&params, |args| {
                    keep(&failure, sink.delete(&handle, Some(args)))
                })
            }),
        )?;
        nsi.set(
            "DeleteAttribute",
            record!(|_, (handle, name): (String, String)| {
                keep(&failure, sink.delete_attribute(&handle, &name))
            }),
        )?;
        nsi.set(
            "SetAttribute",
            record!(|_, (handle, rest): (String, Variadic<Value>)| {
                let params = params_of(rest)?;
                with_args(&params, |args| {
                    keep(&failure, sink.set_attribute(&handle, args))
                })
            }),
        )?;
        nsi.set(
            "SetAttributeAtTime",
            record!(
                |_, (handle, time, rest): (String, f64, Variadic<Value>)| {
                    let params = params_of(rest)?;
                    with_args(&params, |args| {
                        keep(
                            &failure,
                            sink.set_attribute_at_time(&handle, time, args),
                        )
                    })
                }
            ),
        )?;
        nsi.set(
            "Connect",
            record!(|_,
                     (from, from_attr, to, to_attr, rest): (
                String,
                String,
                String,
                String,
                Variadic<Value>,
            )| {
                let params = params_of(rest)?;
                let port = Some(from_attr.as_str()).filter(|p| !p.is_empty());
                with_args(&params, |args| {
                    keep(
                        &failure,
                        sink.connect(&from, port, &to, &to_attr, Some(args)),
                    )
                })
            }),
        )?;
        nsi.set(
            "Disconnect",
            record!(|_,
                     (from, from_attr, to, to_attr): (
                String,
                String,
                String,
                String,
            )| {
                let port = Some(from_attr.as_str()).filter(|p| !p.is_empty());
                keep(&failure, sink.disconnect(&from, port, &to, &to_attr))
            }),
        )?;
        nsi.set(
            "Evaluate",
            record!(|_, rest: Variadic<Value>| {
                let params = params_of(rest)?;
                with_args(&params, |args| keep(&failure, sink.evaluate(args)))
            }),
        )?;

        nsi.set(
            "RenderControl",
            record!(|_, rest: Variadic<Value>| {
                let params = params_of(rest)?;
                let action = params
                    .iter()
                    .find(|p| p.name == "action" && p.type_tag == Type::String)
                    .and_then(|p| p.strings.first())
                    .and_then(|name| action_of(name))
                    .ok_or_else(|| {
                        mlua::Error::runtime(
                            "nsi.RenderControl needs a known action",
                        )
                    })?;
                // ɴsɪ's own `render_control` appends the action, so
                // passing it through here would send it twice.
                let rest: Vec<&Param> =
                    params.iter().filter(|p| p.name != "action").collect();
                let owned: Vec<Param> = rest.into_iter().cloned().collect();
                with_args(&owned, |args| {
                    keep(&failure, sink.render_control(action, Some(args)))
                })
            }),
        )?;

        lua.globals().set("nsi", nsi)?;
        lua.load(source).exec()
    });

    if let Some(error) = failure.into_inner() {
        return Err(Error::Sink(error));
    }
    outcome.map_err(|error| Error::Lua(error.to_string()))
}

/// Record a sink error and unwind the script.
///
/// The error is kept rather than stringified, and the `Err` returned
/// here stops the interpreter: without it the script ran on and the
/// sink kept receiving calls after the refusal, which contradicted what
/// `Error::Sink` promises.
fn keep<E>(
    slot: &RefCell<Option<E>>,
    result: Result<(), E>,
) -> mlua::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(error);
            }
            Err(mlua::Error::runtime("the ɴsɪ sink refused a call"))
        }
    }
}

/// ɴsɪ accepts parameters as variadic tables, or as one table of them.
fn params_of(rest: Variadic<Value>) -> mlua::Result<Vec<Param>> {
    let mut tables: Vec<Table> = Vec::new();
    for value in rest {
        let Value::Table(table) = value else {
            return Err(mlua::Error::runtime(
                "an ɴsɪ parameter must be a table",
            ));
        };
        // A table of parameters, rather than a parameter, has no `name`.
        if table.contains_key("name")? {
            tables.push(table);
        } else {
            for entry in table.sequence_values::<Table>() {
                tables.push(entry?);
            }
        }
    }

    tables.into_iter().map(param_of).collect()
}

/// One `{name=, data=, type=, arraylength=}` table.
fn param_of(table: Table) -> mlua::Result<Param> {
    let name: String = table.get("name")?;
    let array_length: Option<usize> = table.get("arraylength")?;
    let declared: Option<i32> = table.get("type")?;
    let data: Value = table.get("data")?;

    let mut param = Param {
        name,
        type_tag: Type::Invalid,
        array_length: array_length.and_then(NonZeroUsize::new),
        f32s: Vec::new(),
        f64s: Vec::new(),
        i32s: Vec::new(),
        i64s: Vec::new(),
        strings: Vec::new(),
    };

    // ɴsɪ: "the type parameter can be omitted if the parameter is an
    // integer, real or string". Lua 5.4 keeps the two number kinds
    // apart, so the inference is exact rather than a guess.
    let type_tag = match declared {
        Some(value) => tag_of(value)?,
        None => match &data {
            Value::Integer(_) => Type::I32,
            Value::Number(_) => Type::F32,
            Value::String(_) => Type::String,
            _ => {
                return Err(mlua::Error::runtime(
                    "an ɴsɪ parameter with table data needs a type",
                ));
            }
        },
    };
    param.type_tag = type_tag;

    let values: Vec<Value> = match data {
        Value::Table(table) => table
            .sequence_values::<Value>()
            .collect::<mlua::Result<_>>()?,
        scalar => vec![scalar],
    };

    for value in values {
        match type_tag {
            Type::String => param.strings.push(
                value
                    .as_string_lossy()
                    .ok_or_else(|| mlua::Error::runtime("expected a string"))?,
            ),
            Type::I32 => param.i32s.push(integer(&value)? as i32),
            Type::I64 => param.i64s.push(integer(&value)?),
            Type::F64 | Type::MatrixF64 => param.f64s.push(number(&value)?),
            _ => param.f32s.push(number(&value)? as f32),
        }
    }

    // ɴsɪ's tuple types need whole elements; `as_chunks` would drop a
    // remainder, turning a two-value point into an empty one.
    let width = match type_tag {
        Type::Color | Type::Point | Type::Vector | Type::Normal => 3,
        Type::MatrixF32 | Type::MatrixF64 => 16,
        _ => 1,
    };
    let count = match type_tag {
        Type::MatrixF64 => param.f64s.len(),
        _ => param.f32s.len(),
    };
    if width > 1 && !count.is_multiple_of(width) {
        return Err(mlua::Error::runtime(
            "an ɴsɪ tuple parameter needs a whole number of elements",
        ));
    }

    // `arraylength` multiplies the element size; the renderer refuses a
    // parameter whose data does not divide by it, and emitting one
    // produces a stream nothing can read.
    let stride = width * param.array_length.map_or(1, NonZeroUsize::get);
    if stride > 1 && !count.is_multiple_of(stride) {
        return Err(mlua::Error::runtime(
            "an ɴsɪ parameter's data must divide by its arraylength",
        ));
    }

    Ok(param)
}

/// Lua 5.4 keeps integers and floats apart, and both are numbers here.
fn number(value: &Value) -> mlua::Result<f64> {
    match value {
        Value::Integer(integer) => Ok(*integer as f64),
        Value::Number(number) => Ok(*number),
        _ => Err(mlua::Error::runtime("expected a number")),
    }
}

/// The same, kept exact for the integer types.
fn integer(value: &Value) -> mlua::Result<i64> {
    match value {
        Value::Integer(integer) => Ok(*integer),
        Value::Number(number) => Ok(*number as i64),
        _ => Err(mlua::Error::runtime("expected a number")),
    }
}

/// ɴsɪ's `RenderControl` action names.
fn action_of(name: &str) -> Option<Action> {
    Some(match name {
        "start" => Action::Start,
        "stop" => Action::Stop,
        "suspend" => Action::Suspend,
        "resume" => Action::Resume,
        "wait" => Action::Wait,
        "synchronize" => Action::Synchronize,
        _ => return None,
    })
}

fn tag_of(value: i32) -> mlua::Result<Type> {
    Ok(match value {
        1 => Type::F32,
        2 => Type::I32,
        3 => Type::String,
        4 => Type::Color,
        5 => Type::Point,
        6 => Type::Vector,
        7 => Type::Normal,
        8 => Type::MatrixF32,
        0x18 => Type::MatrixF64,
        _ => return Err(mlua::Error::runtime("unknown ɴsɪ type")),
    })
}

/// Build the arguments and hand them to `apply`, borrowing the params.
fn with_args<T>(
    params: &[Param],
    apply: impl FnOnce(&[Arg<'_, 'static>]) -> T,
) -> T {
    let triples: Vec<Vec<[f32; 3]>> = params
        .iter()
        .map(|p| p.f32s.as_chunks::<3>().0.to_vec())
        .collect();
    let matrices_f32: Vec<Vec<[f32; 16]>> = params
        .iter()
        .map(|p| p.f32s.as_chunks::<16>().0.to_vec())
        .collect();
    let matrices_f64: Vec<Vec<[f64; 16]>> = params
        .iter()
        .map(|p| p.f64s.as_chunks::<16>().0.to_vec())
        .collect();
    let borrowed: Vec<Vec<&str>> = params
        .iter()
        .map(|p| p.strings.iter().map(String::as_str).collect())
        .collect();

    let args: Vec<Arg<'_, 'static>> = params
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let data = match p.type_tag {
                Type::F32 => ArgData::from(F32Slice::new(&p.f32s)),
                Type::I32 => ArgData::from(I32Slice::new(&p.i32s)),
                Type::String => {
                    ArgData::from(StringSlice::new(&borrowed[index]))
                }
                Type::Color => ArgData::from(ColorSlice::new(&triples[index])),
                Type::Point => ArgData::from(PointSlice::new(&triples[index])),
                Type::Vector => {
                    ArgData::from(VectorSlice::new(&triples[index]))
                }
                Type::Normal => {
                    ArgData::from(NormalSlice::new(&triples[index]))
                }
                Type::MatrixF32 => {
                    ArgData::from(MatrixF32Slice::new(&matrices_f32[index]))
                }
                Type::MatrixF64 => {
                    ArgData::from(MatrixF64Slice::new(&matrices_f64[index]))
                }
                // `tag_of` yields none of these and inference produces
                // none: ɴsɪ's Lua binding has no name for a double, a
                // 64-bit integer or a pointer.
                // `tag_of` accepts none of these, and inference
                // produces none.
                Type::F64 | Type::I64 | Type::Reference | Type::Invalid => {
                    unreachable!("no Lua spelling yields {:?}", p.type_tag)
                }
            };

            let arg = Arg::new(&p.name, data);
            match p.array_length {
                Some(length) => arg.array_len(length),
                None => arg,
            }
        })
        .collect();

    apply(&args)
}
