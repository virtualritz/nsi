//! A `mesh`'s face structure, with `nholes` applied.
//!
//! ɴsɪ describes faces in two attributes that only mean something
//! together. Without `nholes`, `nvertices` is one count per face. With
//! it, the face count is the number of `nholes` values and each face
//! takes `nholes + 1` values from `nvertices` -- the outer perimeter
//! first, then one per hole.
//!
//! A backend that reads `nvertices` as one-count-per-face therefore
//! draws a *different mesh* for any scene with holes, silently: the
//! counts are all valid, there are simply too many faces and each one
//! is wrong. This resolves the pair once, here, and refuses a scene
//! where they disagree instead of guessing.

use super::*;

/// One face: an outer perimeter, and one perimeter per hole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Face<'a> {
    /// How many vertices the outer perimeter has.
    pub outer: usize,
    /// How many vertices each hole's perimeter has, in order.
    ///
    /// Borrowed from the node's `nvertices`: a Moana-scale mesh has
    /// millions of faces, and a `Vec` each would cost more than the
    /// attribute it describes.
    pub holes: &'a [i32],
}

impl Face<'_> {
    /// Every vertex this face consumes, holes included.
    ///
    /// This is the stride into face-varying data, and into `P.indices`
    /// where one is given.
    #[must_use]
    pub fn vertices(&self) -> usize {
        self.outer
            + self
                .holes
                .iter()
                .map(|count| *count as usize)
                .sum::<usize>()
    }
}

/// The faces of a mesh, in order.
#[derive(Debug, Clone)]
pub struct Faces<'a> {
    counts: &'a [i32],
    holes: Option<&'a [i32]>,
    face: usize,
    taken: usize,
}

impl<'a> Iterator for Faces<'a> {
    type Item = Face<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.holes {
            None => {
                let outer = *self.counts.get(self.face)?;
                self.face += 1;
                Some(Face {
                    outer: outer as usize,
                    holes: &[],
                })
            }
            Some(holes) => {
                let count = *holes.get(self.face)? as usize;
                let outer = *self.counts.get(self.taken)?;
                let holes =
                    &self.counts[self.taken + 1..self.taken + 1 + count];
                self.face += 1;
                self.taken += 1 + count;
                Some(Face {
                    outer: outer as usize,
                    holes,
                })
            }
        }
    }
}

impl Scene {
    /// The faces of a `mesh`, with `nholes` resolved.
    ///
    /// # Errors
    ///
    /// [`ResolveError::MissingFaceCounts`] when the mesh sets no
    /// `nvertices`, which ɴsɪ requires, and
    /// [`ResolveError::MalformedFaceCounts`] when `nholes` asks for
    /// more or fewer `nvertices` values than the mesh carries -- the
    /// case that otherwise renders as a different mesh with no report.
    pub fn faces(&self, handle: &str) -> Result<Faces<'_>, ResolveError> {
        let node = self.node(handle).ok_or_else(|| {
            ResolveError::MissingFaceCounts {
                handle: handle.to_string(),
            }
        })?;
        let counts = node
            .effective(FACE_VERTEX_COUNTS)
            .and_then(OwnedArgument::as_i32s)
            .ok_or_else(|| ResolveError::MissingFaceCounts {
                handle: handle.to_string(),
            })?;
        let holes =
            node.effective(HOLE_COUNTS).and_then(OwnedArgument::as_i32s);

        if let Some(holes) = holes {
            // Each face takes its outer perimeter plus one value per
            // hole; the total is what `nvertices` must carry.
            let expected = holes.len()
                + holes.iter().map(|count| *count as usize).sum::<usize>();
            if expected != counts.len() {
                return Err(ResolveError::MalformedFaceCounts {
                    handle: handle.to_string(),
                    expected,
                    found: counts.len(),
                });
            }
        }

        Ok(Faces {
            counts,
            holes,
            face: 0,
            taken: 0,
        })
    }
}
