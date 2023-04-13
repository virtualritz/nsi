use dl_openvdb_query as vdbq;
use nsi_3delight::*;
use nsi_toolbelt::*;

/// Downloaded from https://jangafx.com/software/embergen/download/free-vdb-animations/
static VDB_ASSET: &str = "assets/embergen_gasoline_explosion_a_50.vdb";

static ENVMAP_HDR: &str = "assets/wooden_lounge_1k.tdl";

pub fn main() {

    println!("{:?}",
                &vdbq::DlOpenVdbQuery::new(VDB_ASSET)
                    .unwrap()
                    .grid_names()
                    .unwrap(),
                );
    let ctx = nsi::Context::new(None).unwrap();

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
            None,
        )
        .0,
    );

    append(
        &ctx,
        ".root",
        None,
        append(
            &ctx,
            &rotation(&ctx, None, 135.0, &[0.0, 1.0, 0.0]),
            None,
            &append(
                &ctx,
                &node(
                    &ctx,
                    None,
                    nsi::node::VOLUME,
                    Some(&[
                        nsi::string!("vdbfilename", VDB_ASSET),
                        nsi::string!("temperaturegrid", "temperature"),
                        nsi::string!("densitygrid", "density"),
                        nsi::string!("emissionintensitygrid", "flames"),
                    ]),
                ),
                Some("geometryattributes"),
                &append(
                    &ctx,
                    &node(&ctx, None, nsi::node::ATTRIBUTES, None),
                    Some("volumeshader"),
                    &node(
                        &ctx,
                        None,
                        nsi::node::SHADER,
                        Some(&[
                            nsi::string!(
                                "shaderfilename",
                                "${DELIGHT}/osl/vdbVolume"
                            ),
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
                            nsi::integers!(
                                "emissionramp_color_curve_Interp",
                                &[3, 3, 3, 3,]
                            )
                            .array_len(4),
                        ]),
                    ),
                )
                .0,
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
                &vdbq::DlOpenVdbQuery::new(VDB_ASSET)
                    .unwrap()
                    .bounding_box()
                    .unwrap(),
            ),
            None,
            // Attach screen to our camera
            &append(
                &ctx,
                &node(
                    &ctx,
                    None,
                    nsi::node::PERSPECTIVE_CAMERA,
                    Some(&[
                        nsi::float!("fov", field_of_view),
                        /*nsi::doubles!("shutterrange", &[-0.01042,
                         * 0.01042]), nsi::doubles!
                         * ("shutteropening", &[0.333, 0.666]), */
                    ]),
                ),
                Some("screens"),
                &append(
                    &ctx,
                    &node(
                        &ctx,
                        None,
                        nsi::node::SCREEN,
                        Some(&[
                            nsi::integers!("resolution", &[1024, 512])
                                .array_len(2),
                            nsi::integer!("oversampling", 64),
                        ]),
                    ),
                    Some("outputlayers"),
                    &append(
                        &ctx,
                        &node(
                            &ctx,
                            None,
                            nsi::node::OUTPUT_LAYER,
                            Some(&[
                                nsi::string!("variablename", "Ci"),
                                nsi::integer!("withalpha", 1),
                                nsi::string!("scalarformat", "float"),
                            ]),
                        ),
                        Some("outputdrivers"),
                        &node(
                            &ctx,
                            Some("driver"),
                            nsi::node::OUTPUT_DRIVER,
                            Some(&[nsi::string!("drivername", "idisplay")]),
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
