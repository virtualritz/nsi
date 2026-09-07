# Data Model: Filtering an ɴsɪ Stream

## `StreamWriter<W: Write>`

An [`Nsi`] sink that writes plain `.nsi` statements.

| Field | Type | Notes |
| --- | --- | --- |
| `out` | `Mutex<W>` | the trait takes `&self` and requires `Send + Sync`; nothing else is held |

`type Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>`, matching
`Recorder` and `Context`, which is what lets one filter forward to any
of them. `type Error = std::io::Error`.

`into_inner()` returns the writer back, so a caller that wrote into a
`Vec<u8>` gets its bytes.

## `LuaWriter<W: Write>`

The same, emitting `nsi.Create(...)` and friends.

## Statement Mapping

| [`Nsi`] method | `.nsi` | Lua |
| --- | --- | --- |
| `create` | `Create "h" "type"` + parameter lines | `nsi.Create("h", "type", {...})` |
| `delete` | `Delete "h"` + parameters | `nsi.Delete("h", {...})` |
| `set_attribute` | `SetAttribute "h"` + parameters | `nsi.SetAttribute("h", {...})` |
| `set_attribute_at_time` | `SetAttributeAtTime "h" <time>` + parameters | `nsi.SetAttributeAtTime("h", t, {...})` |
| `delete_attribute` | `DeleteAttribute "h" "name"` | `nsi.DeleteAttribute("h", "name")` |
| `connect` | `Connect "from" "port" "to" "attr"` + parameters | `nsi.Connect("from", "port", "to", "attr", {...})` |
| `disconnect` | `Disconnect "from" "port" "to" "attr"` | `nsi.Disconnect(...)` |
| `evaluate` | `Evaluate` + parameters | `nsi.Evaluate({...})` |
| `render_control` | `RenderControl` + `"action"` + parameters | `nsi.RenderControl({action = "..."})` |

An unnamed source port is the empty string, which is what ɴsɪ writes
and what the parser reads back as `None`.

## Argument Formatting

Shared with `write_stream` and `write_lua`: each incoming `Arg` becomes
an `OwnedArgument` through `OwnedArgument::from_param`, which applies
ɴsɪ's `count = len / array_length` rule, and is then written by the
same `write_arg`. A `Reference` argument has no text form and its whole
parameter line is omitted, as 3Delight does.

## What a Filter Is

Not a type in this crate: a filter is any [`Nsi`] implementation that
holds another one.

```rust
struct DropLights<N> { inner: N }

impl<N: Nsi> Nsi for DropLights<N> {
    type Arg<'call> = N::Arg<'call>;
    type Error = N::Error;

    fn connect(&self, from: &str, from_attribute: Option<&str>,
               to: &str, to_attribute: &str,
               args: Option<&[Self::Arg<'_>]>) -> Result<(), Self::Error> {
        if to_attribute == "lightset" { return Ok(()); }
        self.inner.connect(from, from_attribute, to, to_attribute, args)
    }
    // the other eight forward
}
```

Returning `Ok(())` without forwarding swallows the call -- 3Delight's
`false` from `nsicallbacks`. Returning `Err` aborts the parse, which
`parse_stream` reports as `Error::Sink`.
