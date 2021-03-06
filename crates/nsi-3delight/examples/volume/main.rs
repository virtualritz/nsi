use dl_openvdb_query as vdbq;
use nsi_3delight::*;
use nsi_core as nsi;
use nsi_toolbelt::*;

static VDB_ASSET: &str = "assets/fireball_hi.vdb";
static ENVMAP_HDR: &str = "assets/wooden_lounge_1k.tdl";

pub fn main() {
    let ctx = nsi::Context::new(&[]).unwrap();

    append(
        &ctx,
        ".root",
        None,
        &environment_texture(
            &ctx,
            None,
            ENVMAP_HDR,
            Some(90.0),
            Some(1.),
            Some(false),
            &[],
        )
        .0,
    );

    /*
    append(&ctx,
        ".root",
        None,
        append(&ctx,
            &rotation(ctx, None, 135.0, &[0.0, 1.0, 0.0]),
            None,
            &append(&ctx.,
                &node(&ctx,
                    None,
                    nsi::NodeType::Volume,
                    &[
                        nsi::string!("vdbfilename", VDB_ASSET),
                        nsi::string!("temperaturegrid", "Ce.x"),
                        nsi::string!("densitygrid", "density"),
                        nsi::string!("velocitygrid", "vel"),
                        nsi::double!("velocityscale", 15.0),
                    ],
                ),
                Some("geometryattributes"),
                &append(&ctx,
                    &node(&ctx, None, nsi::NodeType::Attributes, &[]),
                    Some("volumeshader"),
                    &node(&ctx,
                        None,
                        nsi::NodeType::Shader,
                        &[
                            nsi::string!("shaderfilename", "${DELIGHT}/osl/vdbVolume"),
                            nsi::float!("density", 8.0),
                            nsi::float!("multiple_scattering_intensity", 0.44),
                            nsi::float!("emissionramp_intensity", 1.0),
                            nsi::floats!(
                                "emissionramp_color_curve_Knots",
                                &[0.0, 0.09034268, 0.83800625, 1.0]
                            )
                            .array_len(4),
                            nsi::colors!(
                                "emissionramp_color_curve_Colors",
                                &[
                                    0.,
                                    0.,
                                    0.,
                                    0.,
                                    0.,
                                    0.,
                                    0.832,
                                    0.0416,
                                    0.,
                                    1.,
                                    0.5935334,
                                    0.061999976
                                ]
                            )
                            .array_len(4),
                            nsi::integers!("emissionramp_color_curve_Interp", &[3, 3, 3, 3,])
                                .array_len(4),
                        ],
                    ),
                )
                .0,
            )
            .0,
        )
        .0,
    );*/

    use polyhedron_ops::Polyhedron;

    let polyhedron = Polyhedron::dodecahedron();

    append(
        &ctx,
        ".root",
        None,
        &append(
            &ctx,
            &polyhedron.to_nsi(&ctx, None, None, None, None),
            Some("geometryattributes"),
            &append(
                &ctx,
                &node(&ctx, None, nsi::NodeType::Attributes, &[]),
                Some("surfaceshader"),
                &node(
                    &ctx,
                    None,
                    nsi::NodeType::Shader,
                    &[
                        nsi::string!("shaderfilename", "${DELIGHT}/osl/dlPrincipled"),
                        nsi::color!("i_color", &[1., 0.6, 0.3]),
                        //nsi::arg!("coating_thickness", 0.1),
                        nsi::float!("roughness", 0.1),
                        nsi::float!("specular_level", 1.0),
                        nsi::float!("metallic", 1.),
                        nsi::float!("anisotropy", 0.),
                        nsi::float!("sss_weight", 0.),
                        nsi::color!("sss_color", &[0.5, 0.5, 0.5]),
                        nsi::float!("sss_scale", 0.),
                        nsi::color!("incandescence", &[0., 0., 0.]),
                        nsi::float!("incandescence_intensity", 0.),
                    ],
                ),
            )
            .0,
        )
        .0,
    );

    let field_of_view = 50.0;

    // Build our scene graph.
    // Attach our camera to a look-at xform.
    append(
        &ctx,
        ".root",
        // None – use "objects"
        None,
        &append(
            &ctx,
            &look_at_bounding_box_perspective_camera(
                &ctx,
                None,
                // Direction.
                &[0.0, -0.1, -1.0],
                // Up.
                &[0.0, 1.0, 0.0],
                field_of_view,
                Some(2.0),
                // Bounding box to frame.
                &polyhedron.bounding_box(), /*&vdbq::DlOpenVdbQuery::new(VDB_ASSET)
                                            .unwrap()
                                            .bounding_box()
                                            .unwrap(),*/
            ),
            None,
            // Attach screen to our camera
            &append(
                &ctx,
                &node(
                    &ctx,
                    None,
                    nsi::NodeType::PerspectiveCamera,
                    &[
                        nsi::float!("fov", field_of_view),
                        /*nsi::doubles!("shutterrange", &[-0.01042, 0.01042]),
                         *nsi::doubles!("shutteropening", &[0.333, 0.666]), */
                    ],
                ),
                Some("screens"),
                &append(
                    &ctx,
                    &node(
                        &ctx,
                        None,
                        nsi::NodeType::Screen,
                        &[
                            nsi::integers!("resolution", &[1024, 512]).array_len(2),
                            nsi::integer!("oversampling", 64),
                        ],
                    ),
                    Some("outputlayers"),
                    &append(
                        &ctx,
                        &node(
                            &ctx,
                            None,
                            nsi::NodeType::OutputLayer,
                            &[
                                nsi::string!("variablename", "Ci"),
                                nsi::integer!("withalpha", 1),
                                nsi::string!("scalarformat", "float"),
                            ],
                        ),
                        Some("outputdrivers"),
                        &node(
                            &ctx,
                            Some("driver"),
                            nsi::NodeType::OutputDriver,
                            &[nsi::string!("drivername", "idisplay")],
                        ),
                    )
                    .0,
                )
                .0,
            )
            .0,
        )
        .0,
    );

    ctx.set_attribute(
        ".global",
        &[
            nsi::integer!("renderatlowpriority", 1),
            nsi::string!("bucketorder", "spiral"),
            nsi::integer!("quality.volumesamples", 16),
        ],
    );

    // And now, render it!
    ctx.render_control(&[nsi::string!("action", "start")]);
    ctx.render_control(&[nsi::string!("action", "wait")]);
}
