# Research: Parsing A Stream In Parallel

## D1: barriers, not a sequential fallback

The first plan fell back to the sequential parser for a whole stream
holding any `Delete`, `DeleteAttribute` or `Evaluate`. Treating those --
and `Disconnect` and `RenderControl` -- as barriers between parallel
segments is as correct and loses nothing on the rest of the stream. It
is also what keeps an interactive stream's meaning: edits after a
`RenderControl "synchronize"` belong to the next update, not this one.

## D2: `Disconnect` is a barrier too

A `Connect` and a `Disconnect` of one edge are ordered. Keying the
`Disconnect` like a `Connect` would order it against connections into
the same attribute, but `Disconnect` accepts `.all` in any operand, so
its key cannot be known from its text. A barrier is always right.

## D3: connection order into one attribute matters -- rendered

`nsi.pdf` says `priority` orders connections and that, among
conflicting priorities, the definition closest to the geometry wins. It
says nothing of two connections of **equal** priority into one
attribute. Rendered with `renderdl`: a plane with two `attributes` nodes
connected to its `geometryattributes`, one carrying a red `dlConstant`
surface and one a green, renders **red when the red one is connected
first and green when the green one is** -- the pixel at the centre is
`(255, 0, 0)` and `(0, 255, 0)` respectively.

So `Connect`s keep stream order per destination node and attribute.
Keying them by the whole edge, the first design, would have made the
surface of such an object depend on thread timing.

## D4: 3Delight serialises calls on a context

The request assumed nodes can be created in parallel on one context.
Measured, without the parser: 20 000 `SetAttribute`s on existing meshes
take 11.8 ms from one thread and 21.4 ms from sixteen; 20 000 `Create`s
11.4 ms and 10.8 ms. 3Delight 2.9 takes one call at a time per context,
and contention makes sixteen callers slower than one.

What parallelism buys against 3Delight is therefore parsing overlapped
with the renderer, not calls side by side: 1.15-1.4x on the throughput
corpus. A sink that does take calls concurrently gains up to the
parser's own speed-up, 1.9x.

## D5: the scan was the bottleneck

A first scan reused the lexer and cost 17.7 ms of a 29 ms parallel
parse, 60 %. Skipping strings with `memchr2`, rejecting non-keywords on
their first byte and building no tokens brought it to 9 ms: the whole
parse went from 1.35x to 1.9x. It is still half of the time into a null
sink, and the next step if more is wanted; into 3Delight it is not
what limits.

## D6: errors stay the lexer's

The scan checks nothing: not UTF-8, not escapes, not termination. Each
statement is later parsed by the sequential parser's lexer and `apply`,
so an error is found with the same offset and variant. The one thing
the scan must judge is the stream's first token, since there is no
statement to hand it to; it hands the whole stream to the sequential
parser instead, which fails on that token before any sink call.

## D7: hash collisions are safe

A key is a 64-bit hash of the touched handle, or destination handle and
attribute. Two keys colliding merge two groups, which then run in
stream order: slower, never wrong. So no map is needed; a stable sort
by hash forms the groups.
