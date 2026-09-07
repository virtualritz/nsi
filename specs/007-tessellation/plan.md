# Plan: `nsi-tessellate`

## Approach

A new workspace crate that reads `nsi-intermediate` and calls two
published kernels. It owns the mapping and nothing else: no
subdivision maths, no patch evaluation, no geometry types.

Order of work, cheapest useful thing first:

1. **Subdivision surfaces.** A `mesh` with `subdivision.scheme`,
   through `subdiv-kernels`. This is the case a renderer meets first
   and the one both dependencies are ready for.
2. **Primitive variables through the same stencils**, per the class
   `nsi-intermediate` resolved -- positions are just the first channel.
3. **The two drivers**: a tolerance, and a camera-derived tolerance.
4. **Sparse re-evaluation**, composing `Scene::affected` with
   `subdiv-kernels`'s `affected_outputs`.
5. **`nurbs`**, once R2 has an answer from the renderer.

## Gates

1. **A polygon mesh comes back unchanged.** No `subdivision.scheme`,
   no refinement -- the scene said what the geometry is.
2. **A refined cube is a cube**: the limit surface of a subdivided
   cube has known extents, and a refinement that lost its stencils
   fails them.
3. **Primitive variables land**: a face-varying `st` on the cage is
   still face-varying on the refined mesh, and a per-vertex normal
   follows the vertex stencils. Dropping a channel reddens this.
4. **Both drivers reach the same geometry** for a tolerance and for
   the camera that derives it, which is what makes the no-view path a
   caller rather than a special case.
5. **Refuse rather than guess**: a scheme this crate does not
   implement, or a variable whose class `nsi-intermediate` refused, is
   a typed error.
6. `cargo clippy --all-targets -- -D warnings`, rustdoc, `fmt`, and
   the crate's tests in both `nsi-intermediate` feature
   configurations.

## Artifacts

- [x] `spec.md`
- [x] `plan.md`
- [x] `research.md`
- [ ] `data-model.md`
- [ ] `contracts/tessellation.md`
- [ ] `quickstart.md`
- [ ] `tasks.md`
- [ ] `checklists/requirements.md`

The four unwritten ones are written with the code: their contract rows
need real test names, and writing them first would mean inventing
evidence. `spec.md`, `plan.md` and `research.md` come first because
they are the decisions.

## Out Of Scope

Displacement, curves, volumes, and being on any renderer's default
path -- see `spec.md`.
