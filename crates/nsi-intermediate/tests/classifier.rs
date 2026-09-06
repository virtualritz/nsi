use nsi_intermediate::{EdgeKind, classify};

#[test]
fn scene_membership() {
    assert_eq!(classify(None, "objects"), EdgeKind::SceneMember);
}

#[test]
fn geometry_attributes_dissolve() {
    assert_eq!(
        classify(None, "geometryattributes"),
        EdgeKind::AttributeBinding
    );
}

#[test]
fn surface_shader_is_a_material_reference() {
    assert_eq!(classify(None, "surfaceshader"), EdgeKind::SurfaceShader);
}

#[test]
fn instancing_source_models() {
    assert_eq!(classify(None, "sourcemodels"), EdgeKind::InstanceSource);
}

#[test]
fn output_chain() {
    assert_eq!(classify(None, "screens"), EdgeKind::Screen);
    assert_eq!(classify(None, "outputlayers"), EdgeKind::OutputLayer);
    assert_eq!(classify(None, "outputdrivers"), EdgeKind::OutputDriver);
}

#[test]
fn a_named_output_port_is_a_shader_network_edge() {
    let kind = classify(Some("outColor"), "inColor");
    assert_eq!(
        kind,
        EdgeKind::ShaderNetwork {
            from_port: "outColor".to_string(),
            to_port: "inColor".to_string(),
        }
    );
}

/// The property that matters: an unknown destination must be an error,
/// never a silently-defaulted reference. A misclassified connection does
/// not fail loudly -- it renders, with materials on the wrong shapes.
/// ɴsɪ's destinations are an open set: its own §4.8 connects a node to
/// another's `visibility`, and `facesets` appears in Listing 3.2. So an
/// unlisted destination is carried with its name rather than refused --
/// and, crucially, never *resolved*, so it cannot become a material or
/// an output route by accident. That was the reason the classifier
/// rejected them, and it still holds.
#[test]
fn an_unlisted_destination_is_carried_with_its_name() {
    assert_eq!(
        classify(None, "somethingnobodyimplemented"),
        EdgeKind::Other {
            to_attr: "somethingnobodyimplemented".to_string()
        }
    );
}

/// ɴsɪ documents `Some("")` as equivalent to `None`: both connect the
/// `from` node itself rather than one of its output ports. Treating the
/// empty string as a port name would classify every such connection as a
/// shader network edge, whatever its destination said.
#[test]
fn an_empty_source_port_is_not_a_port() {
    assert_eq!(classify(Some(""), "objects"), EdgeKind::SceneMember);
    assert_eq!(classify(Some(""), "surfaceshader"), EdgeKind::SurfaceShader);
    // And it is still not a shader-network edge.
    assert_eq!(
        classify(Some(""), "nonsense"),
        EdgeKind::Other {
            to_attr: "nonsense".to_string()
        }
    );
}

/// ɴsɪ's documented light-set workflow connects lights to a `set` node,
/// then that node to an `outputlayer`'s `lightset`. Rejecting either
/// destination made the whole workflow unrecordable -- and a MoonRay
/// `LightSet` is exactly this shape.
#[test]
fn set_membership_and_light_sets() {
    assert_eq!(classify(None, "members"), EdgeKind::SetMember);
    assert_eq!(classify(None, "lightset"), EdgeKind::LightSet);
    assert_eq!(
        classify(None, "shaderattributes"),
        EdgeKind::ShaderAttributes
    );
}

/// The ɴsɪ specification declares exactly these `<connection>`
/// attributes. A destination the classifier lacks is a hard stop for the
/// exporter that uses it -- a scene with a lens shader or a background
/// layer could not be recorded at all -- so the list is pinned here
/// rather than grown one bug report at a time.
#[test]
fn every_connection_the_specification_declares_is_classified() {
    for name in [
        "backgroundlayer",
        "bounds",
        "displacementshader",
        "exclusiveshading",
        "geometryattributes",
        "lensshader",
        "lightset",
        "members",
        "objects",
        "outputdrivers",
        "outputlayers",
        "screens",
        "shaderattributes",
        "sourcemodels",
        "surfaceshader",
        "visibility.set.subsurface",
        "volumeshader",
        "facesets",
    ] {
        let kind = classify(None, name);
        assert!(
            !matches!(kind, EdgeKind::Other { .. }),
            "{name} is declared by the specification and must have a name"
        );
        assert_eq!(kind.to_attr(), name, "{name} does not round-trip");
    }
}
