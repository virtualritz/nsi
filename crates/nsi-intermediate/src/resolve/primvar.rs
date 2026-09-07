//! Primitive variables: which value each face-vertex reads.
//!
//! ɴsɪ stores a primitive variable four ways and tells you which only
//! implicitly. The value count decides -- one value is constant, one
//! per face is uniform, one per vertex follows `P`, one per
//! face-vertex is face-varying -- and the `per_face` and `per_vertex`
//! flags exist only to break a tie. The documentation is explicit that
//! they are "only strictly needed in rare circumstances when a
//! geometric primitive's number of vertices matches the number of
//! faces. The most simple case is a tetrahedral mesh which has exactly
//! four vertices and also four faces." On top of that sits indirect
//! lookup: an integer attribute named `<name>.indices` "is read to
//! know which values of the other parameter to use".
//!
//! Every backend must implement all four rules or silently drop `st`,
//! `N` and every user primitive variable -- and a normal read in the
//! wrong interpolation shades plausibly and wrongly, which is worse
//! than dropping it. So this resolves them once, here.
//!
//! # Indices, not values
//!
//! [`PrimitiveVariable::face_varying_indices`] yields the *value
//! index* for each face-vertex rather than the value. The data is
//! whatever ɴsɪ type the variable carries -- `st` is `f32`, an id is
//! `i32`, a name is bytes -- and expanding to values would mean one
//! path per type and a copy of a buffer that is already the largest
//! thing in the scene. A backend indexes its own payload with these.

use super::*;
use nsi_ffi_wrap::nsi_sys::NSIParamFlags;

/// How a primitive variable's values map onto a primitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interpolation {
    /// One value for the whole primitive.
    Constant,
    /// One value per face.
    Uniform,
    /// One value per vertex, indexed as `P` is.
    Vertex,
    /// One value per face-vertex, already in face order.
    FaceVarying,
}

/// One primitive variable, resolved against its primitive.
#[derive(Debug, Clone)]
pub struct PrimitiveVariable<'a> {
    values: &'a OwnedArgument,
    interpolation: Interpolation,
    indices: Option<&'a [i32]>,
    vertex_indices: Option<&'a [i32]>,
    face_vertex_counts: Vec<usize>,
}

impl<'a> PrimitiveVariable<'a> {
    /// The argument the values live in.
    #[must_use]
    pub fn values(&self) -> &'a OwnedArgument {
        self.values
    }

    /// Which of the four classes this variable is.
    #[must_use]
    pub fn interpolation(&self) -> Interpolation {
        self.interpolation
    }

    /// The variable's own `<name>.indices`, if it has one.
    #[must_use]
    pub fn indices(&self) -> Option<&'a [i32]> {
        self.indices
    }

    /// How many face-vertices the primitive has.
    #[must_use]
    pub fn face_vertex_count(&self) -> usize {
        self.face_vertex_counts.iter().sum()
    }

    /// The value index each face-vertex reads, in face order.
    ///
    /// This is the one order every backend wants and none of them
    /// should have to derive: whatever the storage, the result is
    /// `face_vertex_count()` indices into [`PrimitiveVariable::values`].
    pub fn face_varying_indices(&self) -> impl Iterator<Item = usize> + '_ {
        let mut face_vertex = 0usize;
        self.face_vertex_counts
            .iter()
            .enumerate()
            .flat_map(move |(face, count)| {
                let start = face_vertex;
                face_vertex += count;
                (0..*count).map(move |corner| (face, start + corner))
            })
            .map(move |(face, position)| match self.interpolation {
                Interpolation::Constant => 0,
                Interpolation::Uniform => face,
                Interpolation::FaceVarying => self
                    .indices
                    .map_or(position, |indices| indices[position] as usize),
                // A per-vertex variable is indexed the way `P` is: by
                // the mesh's own `P.indices` where there is one, and
                // otherwise face-vertex order, since an unindexed mesh
                // lists its vertices in exactly that order.
                Interpolation::Vertex => self.indices.map_or_else(
                    || {
                        self.vertex_indices.map_or(position, |vertices| {
                            vertices[position] as usize
                        })
                    },
                    |indices| indices[position] as usize,
                ),
            })
    }
}

impl Scene {
    /// Resolve one primitive variable on a `mesh`.
    ///
    /// # Errors
    ///
    /// [`ResolveError::MissingFaceCounts`] and
    /// [`ResolveError::MalformedFaceCounts`] from [`Scene::faces`],
    /// [`ResolveError::MalformedIndices`] when a `<name>.indices` is
    /// not one entry per face-vertex or points outside the values, and
    /// [`ResolveError::AmbiguousInterpolation`] when the value count
    /// matches none of the four classes. Refusing is deliberate: a
    /// guessed interpolation renders, which is how it survives review.
    pub fn primitive_variable(
        &self,
        handle: &str,
        name: &str,
    ) -> Result<Option<PrimitiveVariable<'_>>, ResolveError> {
        let Some(node) = self.node(handle) else {
            return Ok(None);
        };
        let Some(values) = node.effective(name) else {
            return Ok(None);
        };

        let face_vertex_counts: Vec<usize> =
            self.faces(handle)?.map(|face| face.vertices()).collect();
        let face_vertices: usize = face_vertex_counts.iter().sum();
        let faces = face_vertex_counts.len();
        let points = node
            .effective(POSITIONS)
            .map_or(0, OwnedArgument::element_count);
        let vertex_indices = node
            .effective(&format!("{POSITIONS}{INDICES}"))
            .and_then(OwnedArgument::as_i32s);

        let count = values.element_count();
        let indices = node
            .effective(&format!("{name}{INDICES}"))
            .and_then(OwnedArgument::as_i32s);

        // Indirect lookup first: ɴsɪ says the indices say which values
        // to use, and says nothing about how many values there are.
        if let Some(indices) = indices
            && (indices.len() != face_vertices
                || indices.iter().any(|index| *index as usize >= count))
        {
            return Err(ResolveError::MalformedIndices {
                handle: handle.to_string(),
                attribute: name.to_string(),
                indices: indices.len(),
                face_vertices,
                values: count,
            });
        }

        let flags = NSIParamFlags::from_bits_truncate(values.flags);
        // A flag only decides where the count is genuinely ambiguous;
        // it never overrides a count that cannot mean what it claims.
        let per_face = flags.contains(NSIParamFlags::PerFace) && count == faces;
        let per_vertex =
            flags.contains(NSIParamFlags::PerVertex) && count == points;

        // The case the flags exist for: with as many faces as vertices
        // -- the documentation's tetrahedron -- the count says nothing
        // and an unflagged variable means two different things. Choosing
        // one renders, so this refuses instead.
        if count == faces
            && count == points
            && count != face_vertices
            && !per_face
            && !per_vertex
        {
            return Err(ResolveError::AmbiguousInterpolation {
                handle: handle.to_string(),
                attribute: name.to_string(),
                values: count,
                faces,
                vertices: points,
                face_vertices,
            });
        }

        let interpolation = if indices.is_some() {
            Interpolation::FaceVarying
        } else if per_face
            || (!per_vertex && count == faces && count != face_vertices)
        {
            Interpolation::Uniform
        } else if !per_vertex && count == face_vertices {
            Interpolation::FaceVarying
        } else if count == points && points != 0 {
            Interpolation::Vertex
        } else if count == 1 {
            Interpolation::Constant
        } else {
            return Err(ResolveError::AmbiguousInterpolation {
                handle: handle.to_string(),
                attribute: name.to_string(),
                values: count,
                faces,
                vertices: points,
                face_vertices,
            });
        };

        Ok(Some(PrimitiveVariable {
            values,
            interpolation,
            indices,
            vertex_indices,
            face_vertex_counts,
        }))
    }
}
