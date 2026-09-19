# Research: Weld Declarations

## D1: the table, not the shorthand

The draft offers two encodings: the general boundary-use table
(`weld.id`, `weld.kind`, ...) and a shorthand on `nurbs`
(`trim-curves.edge-id`, `stitch.edge-id`) that "lowers to" it. 3Delight
ships neither, and an exporter "can use the general table everywhere".
So the resolver reads the table only; the shorthand can lower to it
later without changing the output types.

## D2: invalid uses are dropped, unverifiable ones kept

The draft calls empty uses, invalid indices and disconnected chains
"invalid declarations", and says a chain whose segments cannot be
checked is not refused. So a use with a real error is left out of
`WeldTable::uses` and reported; a use whose connectivity cannot be
checked from topology -- mixed kinds, partial ranges, curves of
different loops -- is kept and reported as `ConnectivityUnverified`.
A tessellator welds what it can; a backend prints the rest.

## D3: chain checks for trim curves only

Consecutive whole trim curves of one loop are adjacent when the next
index follows the previous one around the loop -- forwards, or
backwards when both are reversed. That is decidable from
`trimcurves.ncurves` alone. Mesh-edge chains need vertex identity,
which an unindexed mesh does not have, so they are left to the
tessellator, which evaluates the boundary anyway.

## D4: grouping by key

The draft's own scale is 10,000 shared boundaries per solid. Grouping by
a linear search over the groups found so far is quadratic, 10^8
comparisons at that size, so `welds()` groups through a hash map.

## D5: vertices are the tessellator's

Welds name edges. A shared corner is where the edges of one loop meet,
linked across faces by the edges they share -- a union-find the
tessellator runs over what this crate exposes. Nothing here compares
positions: the draft says "spatial coincidence alone does not declare a
join".
