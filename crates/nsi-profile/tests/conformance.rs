//! Contract-derived tests for `contracts/profile-conformance.md`.
//!
//! Test names are exactly the ones the contract matrix cites:
//! `resolve_scheme`, `nodedef_completeness`, `validator_violations`,
//! `validator_clean`, `parameter_block_layout`, `edit_classification`.
use std::{env, fs, path::PathBuf};

use nsi_profile::{
    Edit, EditClass, GlslPassthroughEmitter, GpuEmitter, ParamValue,
    PortDefault, PortType, ResolveError, Version,
    edit::classify,
    emit::ShaderLanguage,
    network::{Connection, Network, ShaderNode},
    registry::Registry,
    translate::translate,
    v1::V1_NODES,
    validate::{construct, validate},
    version::PROFILE_V1,
};

/// The conforming fixture: `uv` -> `image` -> `diffuse_bsdf` -> `surface`.
fn conforming_network() -> Network {
    Network::new(
        vec![
            ShaderNode::new("tex_uv", "nsi-profile:uv@1"),
            ShaderNode::new("tex", "nsi-profile:image@1")
                .with_param(
                    "filename",
                    ParamValue::String("assets/checker.tdl".to_string()),
                )
                .with_param(
                    "default_color",
                    ParamValue::Color([0.5, 0.5, 0.5]),
                ),
            ShaderNode::new("mat", "nsi-profile:diffuse_bsdf@1.0")
                .with_param("roughness", ParamValue::Float(0.3)),
            ShaderNode::new("out", "nsi-profile:surface@1")
                .with_param("opacity", ParamValue::Color([1.0, 1.0, 1.0])),
        ],
        vec![
            Connection::new("tex_uv", "out_vector", "tex", "uv"),
            Connection::new("tex", "out_color", "mat", "base_color"),
            Connection::new("mat", "out_bsdf", "out", "bsdf"),
        ],
    )
}

/// The violating fixture: an arbitrary-ᴏsʟ node, an unknown parameter and a
/// type-mismatched connection.
fn violating_network() -> Network {
    Network::new(
        vec![
            ShaderNode::new("custom", "shaders/my_custom.oso"),
            ShaderNode::new("mat", "nsi-profile:diffuse_bsdf@1")
                .with_param("roughnes", ParamValue::Float(0.5)),
            ShaderNode::new("blend", "nsi-profile:mix_color@1"),
            ShaderNode::new("out", "nsi-profile:surface@1"),
        ],
        vec![
            Connection::new("mat", "out_bsdf", "blend", "t"),
            Connection::new("mat", "out_bsdf", "out", "bsdf"),
        ],
    )
}

/// Path of the parameter-block golden file.
fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/parameter_block_v1.txt")
}

/// `nsi-profile:<node>@<version>` resolution on standard `shader` nodes;
/// unknown node/version fails loudly.
#[test]
fn resolve_scheme() {
    let registry = Registry::v1();

    // Major-only and major.minor both resolve.
    let resolved = registry
        .resolve("nsi-profile:diffuse_bsdf@1")
        .expect("`@1` resolves");
    assert_eq!(resolved.node.name, "diffuse_bsdf");
    assert_eq!(resolved.profile.version(), &PROFILE_V1);

    let resolved = registry
        .resolve("nsi-profile:image@1.0")
        .expect("`@1.0` resolves");
    assert_eq!(resolved.node.name, "image");

    // Unknown node.
    assert_eq!(
        registry.resolve("nsi-profile:no_such_node@1"),
        Err(ResolveError::UnknownNode {
            node: "no_such_node".to_string(),
            version_consulted: PROFILE_V1,
        })
    );

    // Unsupported major.
    assert_eq!(
        registry.resolve("nsi-profile:diffuse_bsdf@2"),
        Err(ResolveError::UnsupportedVersion {
            requested: "2".to_string(),
            available: PROFILE_V1,
        })
    );

    // Minor above what this implementation provides: a network validated
    // against 1.7 may use nodes 1.0 does not have.
    assert_eq!(
        registry.resolve("nsi-profile:diffuse_bsdf@1.7"),
        Err(ResolveError::UnsupportedVersion {
            requested: "1.7".to_string(),
            available: PROFILE_V1,
        })
    );

    // Malformed scheme -- prefix present, remainder unparseable.
    assert!(matches!(
        registry.resolve("nsi-profile:diffuse_bsdf"),
        Err(ResolveError::MalformedScheme { .. })
    ));
    assert!(matches!(
        registry.resolve("nsi-profile:@1"),
        Err(ResolveError::MalformedScheme { .. })
    ));
    assert!(matches!(
        registry.resolve("nsi-profile:diffuse_bsdf@one"),
        Err(ResolveError::MalformedScheme { .. })
    ));

    // Not the profile scheme at all -- arbitrary ᴏsʟ.
    assert_eq!(
        registry.resolve("shaders/my_custom.oso"),
        Err(ResolveError::NotProfileScheme {
            shaderfilename: "shaders/my_custom.oso".to_string(),
        })
    );

    // Every failure is a *distinct* variant, not one catch-all.
    assert_ne!(
        registry.resolve("nsi-profile:no_such_node@1"),
        registry.resolve("nsi-profile:diffuse_bsdf@2")
    );
}

/// Every v1 NodeDef has both an ᴏsʟ reference and a GPU source, all ports
/// are typed, and every closure a node names exists in the closure table.
#[test]
fn nodedef_completeness() {
    let registry = Registry::v1();
    let profile = registry.latest().expect("v1 is registered");

    assert_eq!(profile.nodes().len(), 18, "the v1 node table is frozen");
    assert_eq!(profile.nodes().len(), V1_NODES.len());
    assert_eq!(
        profile.closures().len(),
        6,
        "the v1 closure table is frozen"
    );

    profile.nodes().iter().for_each(|node| {
        assert!(
            !node.osl_source.trim().is_empty(),
            "`{}` has no ᴏsʟ reference implementation",
            node.name
        );
        assert!(
            node.osl_source.contains(&format!("nsi_{}", node.name)),
            "`{}`'s ᴏsʟ reference does not declare `nsi_{}`",
            node.name,
            node.name
        );

        assert!(
            !node.glsl_source.trim().is_empty(),
            "`{}` has no GPU source",
            node.name
        );
        assert!(
            node.glsl_source
                .contains(&format!("void nsi_{}(", node.name)),
            "`{}`'s GPU source does not define `nsi_{}`",
            node.name,
            node.name
        );

        assert!(
            !node.description.trim().is_empty(),
            "`{}` has no description",
            node.name
        );

        assert_eq!(
            node.outputs.len(),
            1,
            "`{}` must have exactly one output",
            node.name
        );

        node.inputs.iter().chain(node.outputs).for_each(|port| {
            assert!(
                !port.name.is_empty(),
                "`{}` has an unnamed port",
                node.name
            );
            assert!(
                !port.doc.trim().is_empty(),
                "`{}.{}` is undocumented",
                node.name,
                port.name
            );

            // Closure ports carry no default; every other input does, and
            // its default matches its type.
            match (port.ty, port.default) {
                (PortType::Bsdf | PortType::Surface, default) => assert!(
                    default.is_none(),
                    "`{}.{}` is a closure port with a default",
                    node.name,
                    port.name
                ),
                (PortType::Float, Some(PortDefault::Float(_)))
                | (PortType::Int, Some(PortDefault::Int(_)))
                | (PortType::String, Some(PortDefault::String(_))) => (),
                (
                    PortType::Color
                    | PortType::Vector
                    | PortType::Normal
                    | PortType::Point,
                    Some(PortDefault::Triple(_) | PortDefault::Global(_)),
                ) => (),
                (_, None) => (),
                (ty, default) => panic!(
                    "`{}.{}` is `{ty}` with default {default:?}",
                    node.name, port.name
                ),
            }

            // Enumerant ports list their constants, and the default is one.
            if !port.allowed.is_empty() {
                assert_eq!(port.ty, PortType::String);
                assert!(matches!(
                    port.default,
                    Some(PortDefault::String(text))
                        if port.allowed.contains(&text)
                ));
            }
        });

        node.closures.iter().for_each(|closure| {
            assert!(
                profile.closure(closure).is_some(),
                "`{}` names closure `{closure}`, which is not in the v1 \
                 closure table",
                node.name
            );
        });
    });

    // Every closure the table declares is reachable from some node, and its
    // parameters are fully documented.
    profile.closures().iter().for_each(|closure| {
        assert!(
            profile
                .nodes()
                .iter()
                .any(|node| node.closures.contains(&closure.name)),
            "closure `{}` is unreachable from the v1 node table",
            closure.name
        );
        assert!(!closure.semantic.trim().is_empty());
        assert!(!closure.osl_mapping.trim().is_empty());

        closure.params.iter().for_each(|param| {
            assert!(!param.units.trim().is_empty());
            assert!(!param.range.trim().is_empty());
            assert!(!param.default_desc.trim().is_empty());
        });
    });
}

/// The validator reports out-of-profile constructs with node handle,
/// construct and profile version (US3).
#[test]
fn validator_violations() {
    let registry = Registry::v1();
    let network = violating_network();
    let report = validate(&network, &registry, &PROFILE_V1);

    assert!(!report.is_conforming());
    assert_eq!(report.version_consulted(), &PROFILE_V1);

    let expected = [
        ("custom", construct::NON_PROFILE_SHADERFILENAME),
        ("mat", construct::UNKNOWN_PARAMETER),
        ("blend", construct::PORT_TYPE_MISMATCH),
    ];

    assert_eq!(report.violations().len(), expected.len(), "{report}");

    expected.iter().for_each(|(handle, construct)| {
        let violation = report
            .violations()
            .iter()
            .find(|violation| {
                violation.node_handle == *handle
                    && violation.construct == *construct
            })
            .unwrap_or_else(|| {
                panic!("no `{construct}` violation on `{handle}`:\n{report}")
            });

        // Every violation names the version it was judged against.
        assert_eq!(violation.version_consulted, PROFILE_V1);
        assert!(!violation.detail.is_empty());
    });

    // The CI-log form names all three things per violation.
    let log = report.to_string();
    expected.iter().for_each(|(handle, construct)| {
        assert!(log.contains(handle), "{log}");
        assert!(log.contains(construct), "{log}");
    });
    assert!(log.contains("1.0.0"), "{log}");

    // Translation refuses a non-conforming network rather than stripping it.
    assert!(matches!(
        translate(&network, &registry),
        Err(nsi_profile::Error::NotConforming { .. })
    ));

    // An unregistered version is loud too, on every node.
    let unregistered = Version::new(2, 0, 0);
    let report = validate(&network, &registry, &unregistered);
    assert_eq!(report.violations().len(), network.nodes().len());
    assert!(
        report
            .violations()
            .iter()
            .all(|violation| violation.construct
                == construct::UNSUPPORTED_VERSION)
    );
}

/// A conforming fixture validates clean and translates.
#[test]
fn validator_clean() {
    let registry = Registry::v1();
    let network = conforming_network();
    let report = validate(&network, &registry, &PROFILE_V1);

    assert!(report.is_conforming(), "{report}");
    assert!(report.violations().is_empty());
    assert_eq!(report.to_string(), "profile 1.0.0: conforming");

    let module = translate(&network, &registry).expect("translation succeeds");

    assert_eq!(module.profile_version(), &PROFILE_V1);
    assert_eq!(module.terminal_handle(), "out");
    assert_eq!(module.entry_point(), "nsi_network_main");
    assert_eq!(module.closure_signature(), ["diffuse", "transparent"]);

    // The module carries the shared preamble, every used node function and
    // the entry point -- and nothing the network does not use.
    let source = module.glsl_source();
    assert!(source.starts_with("#version 460"));
    ["nsi_uv(", "nsi_image(", "nsi_diffuse_bsdf(", "nsi_surface("]
        .iter()
        .for_each(|symbol| assert!(source.contains(symbol), "{symbol}"));
    assert!(!source.contains("nsi_metal_bsdf("));
    assert!(source.contains(
        "void nsi_network_main(in NsiShadingContext ctx, out NsiSurface out_surface)"
    ));

    // The texture the scene named got a stable binding.
    assert_eq!(module.textures().len(), 1);
    assert_eq!(module.textures()[0].index, 0);
    assert_eq!(module.textures()[0].filename, "assets/checker.tdl");

    // The built-in emitter hands the module to a backend unchanged.
    let emitted = GlslPassthroughEmitter
        .emit(&module)
        .expect("passthrough emission succeeds");
    assert_eq!(emitted.language, ShaderLanguage::Glsl460);
    assert_eq!(emitted.entry_point, "nsi_network_main");
    assert_eq!(emitted.source(), Some(source));
}

/// The ParameterBlock layout is deterministic and stable within a profile
/// version (R6).
#[test]
fn parameter_block_layout() {
    let registry = Registry::v1();
    let network = conforming_network();

    let first = translate(&network, &registry).expect("translation succeeds");
    let second = translate(&network, &registry).expect("translation succeeds");

    // Determinism: same network, same layout and same module, twice.
    assert_eq!(first.parameter_block(), second.parameter_block());
    assert_eq!(first.glsl_source(), second.glsl_source());

    let layout = first.parameter_block();
    let actual = format!("{layout}\n");
    let path = golden_path();

    if env::var_os("RUST_TEST_UPDATE").is_some() {
        fs::create_dir_all(path.parent().expect("golden file has a parent"))
            .expect("golden directory is writable");
        fs::write(&path, &actual).expect("golden file is writable");
    }

    let expected = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read golden file {}: {error}. Regenerate it with \
             RUST_TEST_UPDATE=1 (human approval required).",
            path.display()
        )
    });

    assert_eq!(actual, expected, "ParameterBlock layout changed");

    // std430 invariants the golden file encodes.
    assert_eq!(layout.total_size() % 16, 0);
    layout.fields().iter().for_each(|field| {
        let align = field.ty.std430_align().expect("block fields are numeric");
        assert_eq!(field.offset % align, 0, "{} is misaligned", field.param);
        assert_eq!(field.size, field.ty.std430_size().expect("sized"));
    });

    // Connected ports, string parameters and geometric defaults are not
    // fields; unconnected numeric ports are.
    assert!(layout.field("mat", "roughness").is_some());
    assert!(layout.field("mat", "base_color").is_none());
    assert!(layout.field("mat", "shading_normal").is_none());
    assert!(layout.field("tex", "filename").is_none());

    // The initial buffer carries the values the scene set.
    let initial = first.initial_parameters();
    let roughness = layout.field("mat", "roughness").expect("field exists");
    assert_eq!(initial.len(), layout.total_size());
    assert!(
        (f32::from_le_bytes(
            initial[roughness.offset..roughness.offset + 4]
                .try_into()
                .expect("four bytes")
        ) - 0.3)
            .abs()
            < f32::EPSILON
    );
}

/// Parameter edits update the ParameterBlock without re-translation;
/// everything else re-translates.
#[test]
fn edit_classification() {
    let registry = Registry::v1();
    let module =
        translate(&conforming_network(), &registry).expect("translation");
    let layout = module.parameter_block();

    // A block parameter: patch bytes at its offset.
    let field = layout.field("mat", "roughness").expect("field exists");
    assert_eq!(
        classify(
            &Edit::SetParam {
                handle: "mat".to_string(),
                param: "roughness".to_string(),
            },
            &module
        ),
        EditClass::ParameterUpdate {
            offset: field.offset,
            size: field.size,
        }
    );

    // A string parameter is baked into the module: re-translate.
    assert_eq!(
        classify(
            &Edit::SetParam {
                handle: "tex".to_string(),
                param: "filename".to_string(),
            },
            &module
        ),
        EditClass::Retranslate
    );

    // A connected port has no field: re-translate.
    assert_eq!(
        classify(
            &Edit::SetParam {
                handle: "mat".to_string(),
                param: "base_color".to_string(),
            },
            &module
        ),
        EditClass::Retranslate
    );

    // Topology edits always re-translate.
    [
        Edit::Connect {
            from_handle: "tex".to_string(),
            from_output: "out_color".to_string(),
            to_handle: "out".to_string(),
            to_input: "opacity".to_string(),
        },
        Edit::Disconnect {
            from_handle: "mat".to_string(),
            from_output: "out_bsdf".to_string(),
            to_handle: "out".to_string(),
            to_input: "bsdf".to_string(),
        },
        Edit::CreateNode {
            handle: "extra".to_string(),
        },
        Edit::DeleteNode {
            handle: "tex".to_string(),
        },
        Edit::SetShaderfilename {
            handle: "mat".to_string(),
        },
    ]
    .iter()
    .for_each(|edit| {
        assert_eq!(classify(edit, &module), EditClass::Retranslate, "{edit:?}");
    });

    // The parameter update is applicable: writing at the classified offset
    // round-trips through the layout.
    let mut buffer = module.initial_parameters().to_vec();
    layout
        .write_param(&mut buffer, "mat", "roughness", &ParamValue::Float(0.75))
        .expect("write succeeds");
    assert_eq!(
        f32::from_le_bytes(
            buffer[field.offset..field.offset + 4]
                .try_into()
                .expect("four bytes")
        ),
        0.75
    );

    // Writing a non-block parameter is a typed error, never a silent no-op.
    assert!(matches!(
        layout.write_param(
            &mut buffer,
            "tex",
            "filename",
            &ParamValue::String("other.tdl".to_string())
        ),
        Err(nsi_profile::Error::NotABlockParameter { .. })
    ));
    assert!(matches!(
        layout.write_param(
            &mut buffer,
            "mat",
            "roughness",
            &ParamValue::Color([1.0, 0.0, 0.0])
        ),
        Err(nsi_profile::Error::ParameterTypeMismatch { .. })
    ));
}
