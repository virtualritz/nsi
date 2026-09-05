use nsi_intermediate::{EdgeKind, classify};

#[test]
fn scene_membership() {
    assert_eq!(classify(None, "objects").unwrap(), EdgeKind::SceneMember);
}

#[test]
fn geometry_attributes_dissolve() {
    assert_eq!(
        classify(None, "geometryattributes").unwrap(),
        EdgeKind::AttributeBinding
    );
}

#[test]
fn surface_shader_is_a_material_reference() {
    assert_eq!(
        classify(None, "surfaceshader").unwrap(),
        EdgeKind::SurfaceShader
    );
}

#[test]
fn instancing_source_models() {
    assert_eq!(
        classify(None, "sourcemodels").unwrap(),
        EdgeKind::InstanceSource
    );
}

#[test]
fn output_chain() {
    assert_eq!(classify(None, "screens").unwrap(), EdgeKind::Screen);
    assert_eq!(
        classify(None, "outputlayers").unwrap(),
        EdgeKind::OutputLayer
    );
    assert_eq!(
        classify(None, "outputdrivers").unwrap(),
        EdgeKind::OutputDriver
    );
}

#[test]
fn a_named_output_port_is_a_shader_network_edge() {
    let kind = classify(Some("outColor"), "inColor").unwrap();
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
#[test]
fn unknown_to_attr_is_rejected() {
    let err = classify(None, "somethingnobodyimplemented").unwrap_err();
    assert!(err.to_string().contains("somethingnobodyimplemented"));
}

/// ɴsɪ documents `Some("")` as equivalent to `None`: both connect the
/// `from` node itself rather than one of its output ports. Treating the
/// empty string as a port name would classify every such connection as a
/// shader network edge, whatever its destination said.
#[test]
fn an_empty_source_port_is_not_a_port() {
    assert_eq!(
        classify(Some(""), "objects").unwrap(),
        EdgeKind::SceneMember
    );
    assert_eq!(
        classify(Some(""), "surfaceshader").unwrap(),
        EdgeKind::SurfaceShader
    );
    // And it still rejects, rather than falling back to a network edge.
    assert!(classify(Some(""), "nonsense").is_err());
}
