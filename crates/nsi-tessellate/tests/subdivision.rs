//! The gates: what a refinement must do, and what it must refuse.

use nsi_intermediate::{OwnedArgument, OwnedData, Scene};
use nsi_tessellate::{Error, Options, refine_variable, subdivision_surface};
use nsi_trait::Type;

/// A unit cube as an ɴsɪ cage: eight vertices, six quads.
fn cube(scheme: Option<&str>) -> Scene {
    let mut scene = Scene::default();
    scene.create("cage", "mesh").unwrap();

    #[rustfmt::skip]
    let positions: Vec<f32> = vec![
        0., 0., 0.,  1., 0., 0.,  1., 1., 0.,  0., 1., 0.,
        0., 0., 1.,  1., 0., 1.,  1., 1., 1.,  0., 1., 1.,
    ];
    #[rustfmt::skip]
    let indices: Vec<i32> = vec![
        0, 3, 2, 1,   4, 5, 6, 7,   0, 1, 5, 4,
        1, 2, 6, 5,   2, 3, 7, 6,   3, 0, 4, 7,
    ];

    let mut attributes = vec![
        OwnedArgument::new("P", Type::Point, 1, 0, OwnedData::F32(positions)),
        OwnedArgument::new(
            "nvertices",
            Type::I32,
            1,
            0,
            OwnedData::I32(vec![4; 6]),
        ),
        OwnedArgument::new(
            "P.indices",
            Type::I32,
            1,
            0,
            OwnedData::I32(indices),
        ),
    ];
    if let Some(scheme) = scheme {
        attributes.push(OwnedArgument::new(
            "subdivision.scheme",
            Type::String,
            1,
            0,
            OwnedData::String(vec![scheme.as_bytes().to_vec()]),
        ));
    }
    scene.set_attribute("cage", attributes).unwrap();
    scene
}

/// A refined cube is still a cube: every point stays inside the cage's
/// bounds, the shape shrinks towards the limit surface rather than
/// growing, and the face count follows the level.
#[test]
fn a_refined_cube_is_a_cube() {
    let scene = cube(Some("catmull-clark"));
    let refined =
        subdivision_surface(&scene, "cage", &Options::default()).unwrap();

    // Two levels of Catmull-Clark: each quad becomes four, twice.
    assert_eq!(refined.face_vertex_counts().len(), 6 * 4 * 4);
    assert!(
        refined.face_vertex_counts().iter().all(|count| *count == 4),
        "Catmull-Clark makes quads",
    );

    let inside = refined.positions().iter().all(|point| {
        point.iter().all(|value| (-0.001..=1.001).contains(value))
    });
    assert!(inside, "the limit surface stays within the convex hull");

    // A cube's limit surface is strictly smaller than its cage: the
    // corner at (0,0,0) pulls in towards the centre.
    let nearest = refined
        .positions()
        .iter()
        .map(|point| point[0] + point[1] + point[2])
        .fold(f32::INFINITY, f32::min);
    assert!(nearest > 0.01, "the corners pull in, they do not stay put");
}

/// The level decides the density, and that is what a tolerance maps
/// onto.
#[test]
fn more_levels_means_more_faces() {
    let scene = cube(Some("catmull-clark"));
    let coarse = subdivision_surface(
        &scene,
        "cage",
        &Options {
            levels: core::num::NonZeroU8::new(1).unwrap(),
        },
    )
    .unwrap();
    let fine = subdivision_surface(
        &scene,
        "cage",
        &Options {
            levels: core::num::NonZeroU8::new(3).unwrap(),
        },
    )
    .unwrap();

    assert_eq!(coarse.face_vertex_counts().len(), 6 * 4);
    assert_eq!(fine.face_vertex_counts().len(), 6 * 4 * 4 * 4);
}

/// An edge-length target picks a level without a camera anywhere near
/// it -- the 3D-printing path.
#[test]
fn an_edge_length_target_picks_a_level() {
    // A one-unit edge to a quarter unit is two halvings.
    assert_eq!(Options::for_edge_length(1.0, 0.25, 8).levels.get(), 2);
    // Already fine enough is still one level, never zero.
    assert_eq!(Options::for_edge_length(0.1, 1.0, 8).levels.get(), 1);
    // And the limit is a limit.
    assert_eq!(Options::for_edge_length(1000.0, 0.001, 3).levels.get(), 3);
}

/// A polygon mesh is the geometry, not a cage for one. Refining it
/// would answer a question the scene did not ask.
#[test]
fn a_polygon_mesh_is_refused() {
    let scene = cube(None);
    assert!(matches!(
        subdivision_surface(&scene, "cage", &Options::default()),
        Err(Error::NotASubdivisionSurface { .. })
    ));
}

/// A scheme this crate does not implement is named, not guessed at.
#[test]
fn an_unimplemented_scheme_is_refused() {
    let scene = cube(Some("loop"));
    assert!(matches!(
        subdivision_surface(&scene, "cage", &Options::default()),
        Err(Error::UnknownScheme { .. })
    ));
}

/// A per-vertex variable follows the vertex stencils, exactly as the
/// positions do -- so a colour painted on the cage lands on the
/// refined surface instead of being dropped.
#[test]
fn a_per_vertex_variable_refines_with_the_surface() {
    let mut scene = cube(Some("catmull-clark"));
    scene
        .set_attribute(
            "cage",
            vec![OwnedArgument::new(
                "Cs",
                Type::Color,
                1,
                0,
                // One colour per vertex, matching `P`.
                OwnedData::F32((0..8 * 3).map(|i| i as f32 / 24.0).collect()),
            )],
        )
        .unwrap();

    let options = Options::default();
    let refined = subdivision_surface(&scene, "cage", &options).unwrap();
    let colours = refine_variable(&scene, "cage", "Cs", &options)
        .unwrap()
        .expect("the variable is there");

    assert_eq!(
        colours.len(),
        refined.positions().len(),
        "one colour per refined vertex",
    );
    assert!(
        colours
            .iter()
            .flatten()
            .all(|value| (-0.001..=1.001).contains(value)),
        "interpolated, not extrapolated",
    );
}

/// An attribute the cage does not carry is absent, not an error.
#[test]
fn an_absent_variable_is_none() {
    let scene = cube(Some("catmull-clark"));
    assert!(
        refine_variable(&scene, "cage", "st", &Options::default())
            .unwrap()
            .is_none()
    );
}
