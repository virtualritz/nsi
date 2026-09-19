//! Assembly placements: where each shell of a STEP file sits in the world.
//!
//! Lifted from `monster-step-viewer`'s `src/step_loader/parsing.rs`
//! (`parse_assembly_transforms` and its helpers) and
//! `src/step_loader/transform.rs` (`Transform`), minus what only the
//! viewer's display needs.
use std::collections::{HashMap, HashSet};

/// A 4×4 transformation matrix, column-major.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    /// Column-major storage: [col0, col1, col2, col3].
    pub cols: [[f64; 4]; 4],
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            cols: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create a transform from AXIS2_PLACEMENT_3D components.
    pub fn from_axis2_placement(
        location: [f64; 3],
        axis: [f64; 3],
        ref_dir: [f64; 3],
    ) -> Self {
        // axis is Z direction, ref_dir is X direction.
        // Y = Z cross X.
        let z = normalize(axis);
        let x = normalize(ref_dir);
        let y = cross(z, x);

        Self {
            cols: [
                [x[0], x[1], x[2], 0.0],
                [y[0], y[1], y[2], 0.0],
                [z[0], z[1], z[2], 0.0],
                [location[0], location[1], location[2], 1.0],
            ],
        }
    }

    /// Multiply two transforms: self * other.
    pub fn mul(&self, other: &Transform) -> Transform {
        let mut result = [[0.0; 4]; 4];
        for (i, result_col) in result.iter_mut().enumerate() {
            for (j, result_elem) in result_col.iter_mut().enumerate() {
                for k in 0..4 {
                    *result_elem += self.cols[k][j] * other.cols[i][k];
                }
            }
        }
        Transform { cols: result }
    }

    /// Compute the inverse transform.
    pub fn inverse(&self) -> Transform {
        // For a rigid transform (rotation + translation), inverse is:
        // R^-1 = R^T, t^-1 = -R^T * t.
        let r00 = self.cols[0][0];
        let r01 = self.cols[1][0];
        let r02 = self.cols[2][0];
        let r10 = self.cols[0][1];
        let r11 = self.cols[1][1];
        let r12 = self.cols[2][1];
        let r20 = self.cols[0][2];
        let r21 = self.cols[1][2];
        let r22 = self.cols[2][2];
        let tx = self.cols[3][0];
        let ty = self.cols[3][1];
        let tz = self.cols[3][2];

        // R^T.
        let inv_tx = -(r00 * tx + r10 * ty + r20 * tz);
        let inv_ty = -(r01 * tx + r11 * ty + r21 * tz);
        let inv_tz = -(r02 * tx + r12 * ty + r22 * tz);

        Transform {
            cols: [
                [r00, r01, r02, 0.0],
                [r10, r11, r12, 0.0],
                [r20, r21, r22, 0.0],
                [inv_tx, inv_ty, inv_tz, 1.0],
            ],
        }
    }

    /// The matrix as ɴsɪ's `transformationmatrix` wants it.
    ///
    /// ɴsɪ uses row vectors, this type column vectors: the two matrices
    /// are transposes of each other, so the column-major storage read
    /// out in order is ɴsɪ's row-major layout (the viewer's
    /// `mat4_to_nsi`).
    pub fn to_nsi(self) -> [f64; 16] {
        let mut matrix = [0.0; 16];
        self.cols
            .iter()
            .flatten()
            .zip(matrix.iter_mut())
            .for_each(|(value, slot)| *slot = *value);
        matrix
    }
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-10 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Preprocess STEP file to join multi-line entities.
fn preprocess_step_entities(raw: &str) -> String {
    // STEP entities can span multiple lines, ending with ;.
    // Join lines to make parsing easier.
    let mut result = String::with_capacity(raw.len());
    for line in raw.lines() {
        let line = line.trim();
        if !line.is_empty() {
            if !result.is_empty() && !result.ends_with(';') {
                result.push(' ');
            }
            result.push_str(line);
        }
    }
    result
}

/// Parse assembly transforms from raw STEP file content.
/// Returns a map from shell entity ID to world transform.
/// Uses foxtrot's approach: build parent->child graph, detect roots, traverse
/// top-down.
pub fn parse_assembly_transforms(raw: &str) -> HashMap<u64, Transform> {
    // Parse basic geometric entities.
    let mut cartesian_points: HashMap<u64, [f64; 3]> = HashMap::default();
    let mut directions: HashMap<u64, [f64; 3]> = HashMap::default();
    let mut placement_refs: HashMap<u64, (u64, u64, u64)> = HashMap::default();
    // ITEM_DEFINED_TRANSFORMATION: id -> (from_placement, to_placement).
    let mut item_transforms: HashMap<u64, (u64, u64)> = HashMap::default();
    // REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION: (rep_1, rep_2,
    // transform_id).
    let mut rep_relationships: Vec<(u64, u64, u64)> = Vec::new();
    // MANIFOLD_SOLID_BREP: manifold_id -> shell_id.
    let mut manifold_to_shell: HashMap<u64, u64> = HashMap::default();
    // ADVANCED_BREP_SHAPE_REPRESENTATION: absr_id -> Vec<all refs including
    // manifolds>.
    let mut absr_refs: HashMap<u64, Vec<u64>> = HashMap::default();
    // SHAPE_REPRESENTATION_RELATIONSHIP: (rep_1, rep_2) - links reps.
    let mut shape_rep_relationships: Vec<(u64, u64)> = Vec::new();

    // Preprocess: join multi-line entities.
    let joined = preprocess_step_entities(raw);

    // First pass: collect all entity definitions.
    for entity in joined.split(';') {
        let entity = entity.trim();
        let Some(rest) = entity.strip_prefix('#') else {
            continue;
        };
        let Some((id_str, rest)) = rest.split_once('=') else {
            continue;
        };
        let Ok(id) = id_str.trim().parse::<u64>() else {
            continue;
        };
        let rest = rest.trim();

        if rest.starts_with("CARTESIAN_POINT") {
            if let Some(coords) = parse_point_coords(rest) {
                cartesian_points.insert(id, coords);
            }
        } else if rest.starts_with("DIRECTION") {
            if let Some(coords) = parse_point_coords(rest) {
                directions.insert(id, coords);
            }
        } else if rest.starts_with("AXIS2_PLACEMENT_3D") {
            let refs = parse_hash_refs(rest);
            if refs.len() >= 3 {
                placement_refs.insert(id, (refs[0], refs[1], refs[2]));
            }
        } else if rest.starts_with("ITEM_DEFINED_TRANSFORMATION") {
            let refs = parse_hash_refs(rest);
            if refs.len() >= 2 {
                item_transforms.insert(id, (refs[0], refs[1]));
            }
        } else if rest
            .contains("REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION")
        {
            let refs = parse_hash_refs(rest);
            if refs.len() >= 3 {
                rep_relationships.push((refs[0], refs[1], refs[2]));
            }
        } else if rest.starts_with("ADVANCED_BREP_SHAPE_REPRESENTATION") {
            let refs = parse_hash_refs(rest);
            absr_refs.insert(id, refs);
        } else if rest.starts_with("MANIFOLD_SOLID_BREP") {
            let refs = parse_hash_refs(rest);
            if !refs.is_empty() {
                manifold_to_shell.insert(id, refs[0]);
            }
        } else if rest.starts_with("SHAPE_REPRESENTATION_RELATIONSHIP")
            && !rest.contains("REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION")
        {
            let refs = parse_hash_refs(rest);
            if refs.len() >= 2 {
                shape_rep_relationships.push((refs[0], refs[1]));
            }
        }
    }

    // Resolve AXIS2_PLACEMENT_3D.
    let resolved_placements: HashMap<u64, Transform> = placement_refs
        .iter()
        .map(|(&id, &(loc_id, axis_id, ref_id))| {
            let location = cartesian_points
                .get(&loc_id)
                .copied()
                .unwrap_or([0.0, 0.0, 0.0]);
            let axis =
                directions.get(&axis_id).copied().unwrap_or([0.0, 0.0, 1.0]);
            let ref_dir =
                directions.get(&ref_id).copied().unwrap_or([1.0, 0.0, 0.0]);
            (id, Transform::from_axis2_placement(location, axis, ref_dir))
        })
        .collect();

    // ====== FOXTROT-STYLE TOP-DOWN TRANSFORM TRAVERSAL ======.

    // Step 1: Build transform stack (parent -> [(child, transform)]).
    let transform_stack = build_transform_stack(
        &rep_relationships,
        &item_transforms,
        &resolved_placements,
    );

    // Step 2: Build shape_rep_relationship map for traversal.
    let mut shape_rep_map: HashMap<u64, Vec<u64>> = HashMap::default();
    for &(r1, r2) in &shape_rep_relationships {
        shape_rep_map.entry(r1).or_default().push(r2);
        shape_rep_map.entry(r2).or_default().push(r1);
    }

    // Step 3: Find roots and traverse top-down.
    let roots = find_transform_roots(&transform_stack);

    // Depth-first traversal from roots, accumulating transforms.
    let mut rep_transforms: HashMap<u64, Transform> = HashMap::default();
    let mut todo: Vec<(u64, Transform)> = roots
        .into_iter()
        .map(|r| (r, Transform::identity()))
        .collect();

    while let Some((rep_id, mat)) = todo.pop() {
        // Follow shape_rep_relationships (no transform change).
        if let Some(linked) = shape_rep_map.get(&rep_id) {
            for &child in linked {
                if !rep_transforms.contains_key(&child) {
                    todo.push((child, mat));
                }
            }
        }

        // Follow transform_stack (with transform).
        if let Some(children) = transform_stack.get(&rep_id) {
            for &(child, ref child_mat) in children {
                if !rep_transforms.contains_key(&child) {
                    let combined = mat.mul(child_mat);
                    todo.push((child, combined));
                }
            }
        }

        // Store this rep's transform if it's a leaf (ABSR or similar).
        rep_transforms.insert(rep_id, mat);
    }

    // Step 4: Map manifolds to their ABSR's transform.
    let mut manifold_to_absr: HashMap<u64, u64> = HashMap::default();
    for (&absr_id, refs) in &absr_refs {
        for &ref_id in refs {
            if manifold_to_shell.contains_key(&ref_id) {
                manifold_to_absr.insert(ref_id, absr_id);
            }
        }
    }

    // Step 5: Build shell -> world transform.
    let mut shell_transforms: HashMap<u64, Transform> = manifold_to_shell
        .iter()
        .filter_map(|(manifold_id, &shell_id)| {
            let absr_id = manifold_to_absr.get(manifold_id)?;
            Some((shell_id, *rep_transforms.get(absr_id)?))
        })
        .collect();

    // If no transforms found via hierarchy, shells get identity.
    if shell_transforms.is_empty() {
        for &shell_id in manifold_to_shell.values() {
            shell_transforms.insert(shell_id, Transform::identity());
        }
    }

    shell_transforms
}

/// Type of the transform stack: parent_rep -> [(child_rep, transform)].
type TransformStack = HashMap<u64, Vec<(u64, Transform)>>;

/// Build transform stack: parent_rep -> [(child_rep, transform)].
fn build_transform_stack(
    rep_relationships: &[(u64, u64, u64)],
    item_transforms: &HashMap<u64, (u64, u64)>,
    placements: &HashMap<u64, Transform>,
) -> TransformStack {
    // Try normal direction first (rep_1 is parent, rep_2 is child).
    let stack = build_transform_stack_directed(
        rep_relationships,
        item_transforms,
        placements,
        false,
    );
    let roots = find_transform_roots(&stack);

    // If multiple roots, flip direction (like foxtrot does).
    if roots.len() > 1 {
        let flipped_stack = build_transform_stack_directed(
            rep_relationships,
            item_transforms,
            placements,
            true,
        );
        let flipped_roots = find_transform_roots(&flipped_stack);
        if flipped_roots.len() < roots.len() {
            return flipped_stack;
        }
    }

    stack
}

fn build_transform_stack_directed(
    rep_relationships: &[(u64, u64, u64)],
    item_transforms: &HashMap<u64, (u64, u64)>,
    placements: &HashMap<u64, Transform>,
    flip: bool,
) -> TransformStack {
    let mut stack = TransformStack::default();

    for &(rep_1, rep_2, transform_id) in rep_relationships {
        let (parent, child) =
            if flip { (rep_1, rep_2) } else { (rep_2, rep_1) };

        // Compute the transform from ITEM_DEFINED_TRANSFORMATION.
        let mut mat =
            compute_item_transform(transform_id, item_transforms, placements);
        if flip {
            mat = mat.inverse();
        }

        stack.entry(parent).or_default().push((child, mat));
    }

    stack
}

/// Find roots: representations that are parents but never children.
fn find_transform_roots(stack: &TransformStack) -> Vec<u64> {
    let children: HashSet<u64> = stack
        .values()
        .flat_map(|v| v.iter().map(|(c, _)| *c))
        .collect();

    stack
        .keys()
        .filter(|k| !children.contains(k))
        .copied()
        .collect()
}

/// Compute transform from ITEM_DEFINED_TRANSFORMATION.
/// Formula: t2 * inverse(t1) where t1=transform_item_1, t2=transform_item_2.
fn compute_item_transform(
    transform_id: u64,
    item_transforms: &HashMap<u64, (u64, u64)>,
    placements: &HashMap<u64, Transform>,
) -> Transform {
    let Some(&(place_1_id, place_2_id)) = item_transforms.get(&transform_id)
    else {
        return Transform::identity();
    };

    let t1 = placements.get(&place_1_id).copied().unwrap_or_default();
    let t2 = placements.get(&place_2_id).copied().unwrap_or_default();

    // t2 * inverse(t1): converts from local frame (t1) to target frame (t2).
    t2.mul(&t1.inverse())
}

/// Parse #id references from a STEP entity string.
fn parse_hash_refs(s: &str) -> Vec<u64> {
    let mut refs = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '#' {
            let mut num = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    num.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            if let Ok(id) = num.parse::<u64>() {
                refs.push(id);
            }
        }
    }
    refs
}

/// Parse coordinate values from CARTESIAN_POINT or DIRECTION.
/// Format varies: CARTESIAN_POINT('',(x,y,z)) or CARTESIAN_POINT ( '', ( x, y,
/// z ) ).
fn parse_point_coords(s: &str) -> Option<[f64; 3]> {
    // Find the coordinate tuple - whitespace varies between files.
    // Look for the comma after the name string, then find the opening paren.
    let comma_pos = s.find(',')?;
    let after_comma = &s[comma_pos + 1..];
    let paren_pos = after_comma.find('(')?;
    let inner = &after_comma[paren_pos + 1..];
    let end = inner.find(')')?;
    let coords_str = &inner[..end];

    let parts: Vec<&str> = coords_str.split(',').collect();
    if parts.len() < 3 {
        return None;
    }

    let x = parse_step_float(parts[0])?;
    let y = parse_step_float(parts[1])?;
    let z = parse_step_float(parts[2])?;

    Some([x, y, z])
}

/// Parse a STEP float value (handles E notation like "0.E+000").
fn parse_step_float(s: &str) -> Option<f64> {
    let s = s.trim();
    // Handle STEP's weird notation like "0.E+000" (missing digit after
    // decimal).
    let s = if s.contains(".E") {
        s.replace(".E", ".0E")
    } else {
        s.to_string()
    };
    s.parse().ok()
}
