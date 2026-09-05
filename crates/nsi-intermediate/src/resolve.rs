//! Graph rewrites: turning ɴsɪ's scene-graph semantics into the flat
//! facts a renderer wants.
//!
//! Both target renderers need this and neither should re-derive it.
//! Mitsuba has no transform tree, only a `to_world` per shape; MoonRay
//! resolves geometry to world space too. So the chain has to be
//! composed here, once.

use crate::{EdgeKind, OwnedData, Scene};

/// A 4x4 identity, row-major.
#[rustfmt::skip]
pub const IDENTITY: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
];

/// The ɴsɪ attribute holding a transform node's matrix.
const TRANSFORMATION_MATRIX: &str = "transformationmatrix";

/// What an ɴsɪ `attributes` node resolves to for one piece of geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The `attributes` node handle. Its remaining attributes --
    /// visibility flags in particular -- are read by the backend, which
    /// knows how its renderer encodes them.
    pub attributes: String,
    /// The shader reached through `surfaceshader`, when there is one.
    /// An `attributes` node carrying only visibility has none.
    pub surface_shader: Option<String>,
}

/// One renderable output: a camera paired with a screen, and the AOVs
/// written from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutput {
    pub camera: String,
    /// The screen, which is what carries resolution and oversampling.
    pub screen: String,
    /// AOVs in connection order; may be empty.
    pub layers: Vec<OutputLayer>,
}

/// One AOV and the drivers it is written to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLayer {
    pub handle: String,
    /// A layer may fan out to several drivers -- a file and a display,
    /// say -- so this is a list, in connection order.
    pub drivers: Vec<String>,
}

/// Row-major 4x4 product, `a` then `b`.
///
/// ɴsɪ uses the RenderMan row-vector convention: a point is a row and
/// transforms multiply on the right, so `p * a * b` applies `a` first.
/// Composing a child with its parent is therefore `mul(child, parent)`,
/// not the other way round.
fn mul(a: [f64; 16], b: [f64; 16]) -> [f64; 16] {
    let mut out = [0.0; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}

impl Scene {
    /// Compose the transform chain applying to `handle`, from the node
    /// itself up to `.root`.
    ///
    /// Includes `handle`'s own matrix when it is a transform node, so
    /// asking about a transform and asking about its child differ by
    /// exactly that node's matrix.
    ///
    /// Non-`f64` matrices are ignored: ɴsɪ's `transformationmatrix` is
    /// documented as `doublematrix`, and silently reinterpreting an
    /// `f32` one would be worse than skipping it.
    pub fn world_transform(&self, handle: &str) -> [f64; 16] {
        let mut matrix = IDENTITY;
        let mut current = handle;
        // ɴsɪ does not forbid a cycle. Bound the walk by the node count
        // so a malformed scene cannot hang a render.
        let mut budget = self.nodes.len() + 1;

        loop {
            if let Some(local) = self.local_transform(current) {
                matrix = mul(matrix, local);
            }

            budget = match budget.checked_sub(1) {
                Some(remaining) if remaining > 0 => remaining,
                _ => break,
            };

            let parent = self
                .edges
                .iter()
                .find(|edge| edge.from == current && edge.kind == EdgeKind::SceneMember);

            match parent {
                Some(edge) if edge.to != crate::ROOT => current = &edge.to,
                _ => break,
            }
        }

        matrix
    }

    /// Dissolve the `attributes` node bound to a piece of geometry.
    ///
    /// ɴsɪ routes material through an intermediate node —
    /// `shader -> attributes -> geometry` — that neither target renderer
    /// has. Mitsuba wants a `bsdf` on the shape; MoonRay wants a `Layer`
    /// entry. Both need the same two-hop walk, so it happens here once.
    ///
    /// One `attributes` node may bind to many shapes, so this resolves
    /// per geometry rather than producing a single owner.
    ///
    /// Returns `None` for geometry with no `attributes` bound. The
    /// attributes handle is returned rather than its contents because
    /// what else lives on that node — visibility flags above all — is
    /// encoded differently by each renderer, and inventing a common
    /// shape for it here would be guesswork.
    pub fn geometry_binding(&self, geometry: &str) -> Option<Binding> {
        let attributes = self
            .edges
            .iter()
            .find(|edge| edge.to == geometry && edge.kind == EdgeKind::AttributeBinding)
            .map(|edge| edge.from.clone())?;

        let surface_shader = self
            .edges
            .iter()
            .find(|edge| edge.to == attributes && edge.kind == EdgeKind::SurfaceShader)
            .map(|edge| edge.from.clone());

        Some(Binding {
            attributes,
            surface_shader,
        })
    }

    /// Resolve ɴsɪ's output chain into what a renderer actually needs.
    ///
    /// ɴsɪ spreads this over four nodes and three connection classes —
    /// `outputdriver -> outputlayer -> screen -> camera` — where both
    /// targets want it collapsed: Mitsuba into a `Sensor` with a `Film`,
    /// MoonRay into `RenderOutput`s. The walk is the same either way.
    ///
    /// One entry per screen, since a screen is what pairs a camera with
    /// a resolution. A screen with no layers still yields an entry: the
    /// camera and resolution are meaningful on their own.
    ///
    /// Layers and drivers come back in connection order, which is
    /// insertion order in `edges`, so AOV order is the order the
    /// consumer declared.
    pub fn render_outputs(&self) -> Vec<RenderOutput> {
        self.edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Screen)
            .map(|screen_edge| {
                let screen = &screen_edge.from;
                let layers = self
                    .edges
                    .iter()
                    .filter(|edge| edge.to == *screen && edge.kind == EdgeKind::OutputLayer)
                    .map(|layer_edge| OutputLayer {
                        handle: layer_edge.from.clone(),
                        drivers: self
                            .edges
                            .iter()
                            .filter(|edge| {
                                edge.to == layer_edge.from && edge.kind == EdgeKind::OutputDriver
                            })
                            .map(|edge| edge.from.clone())
                            .collect(),
                    })
                    .collect();

                RenderOutput {
                    camera: screen_edge.to.clone(),
                    screen: screen.clone(),
                    layers,
                }
            })
            .collect()
    }

    /// The prototypes an `instances` node draws from, in connection
    /// order.
    ///
    /// Mitsuba turns these into a `shapegroup` referenced by `instance`
    /// shapes; MoonRay into a `GeometrySet`. Both start from this list.
    pub fn instance_sources(&self, instances: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter(|edge| edge.to == instances && edge.kind == EdgeKind::InstanceSource)
            .map(|edge| edge.from.clone())
            .collect()
    }

    /// This node's own matrix, if it is a transform carrying one.
    fn local_transform(&self, handle: &str) -> Option<[f64; 16]> {
        let node = self.nodes.get(handle)?;
        let arg = node.attrs.get(TRANSFORMATION_MATRIX)?;
        match &arg.data {
            OwnedData::F64(values) if values.len() == 16 => {
                let mut matrix = [0.0; 16];
                matrix.copy_from_slice(values);
                Some(matrix)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{OwnedArg, OwnedData, Scene};
    use nsi_trait::Type;

    /// A 4x4 row-major translation, the shape ɴsɪ stores in
    /// `transformationmatrix`.
    fn translate(x: f64, y: f64, z: f64) -> OwnedArg {
        #[rustfmt::skip]
        let m = vec![
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
              x,   y,   z, 1.0,
        ];
        OwnedArg {
            name: "transformationmatrix".to_string(),
            type_tag: Type::MatrixF64,
            array_length: 1,
            flags: 0,
            data: OwnedData::F64(m),
        }
    }

    fn scale(s: f64) -> OwnedArg {
        #[rustfmt::skip]
        let m = vec![
              s, 0.0, 0.0, 0.0,
            0.0,   s, 0.0, 0.0,
            0.0, 0.0,   s, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        OwnedArg {
            name: "transformationmatrix".to_string(),
            type_tag: Type::MatrixF64,
            array_length: 1,
            flags: 0,
            data: OwnedData::F64(m),
        }
    }

    #[test]
    fn a_node_with_no_transforms_is_identity() {
        let mut scene = Scene::default();
        scene.create("mesh", "mesh");
        assert_eq!(scene.world_transform("mesh"), super::IDENTITY);
    }

    #[test]
    fn a_single_transform_applies_to_its_child() {
        let mut scene = Scene::default();
        scene.create("xf", "transform");
        scene.set_attribute("xf", vec![translate(1.0, 2.0, 3.0)]);
        scene.create("mesh", "mesh");
        scene.connect("mesh", None, "xf", "objects").unwrap();
        scene.connect("xf", None, ".root", "objects").unwrap();

        let m = scene.world_transform("mesh");
        assert_eq!(&m[12..15], &[1.0, 2.0, 3.0]);
    }

    /// Nested translations accumulate.
    #[test]
    fn nested_transforms_compose() {
        let mut scene = Scene::default();
        scene.create("outer", "transform");
        scene.set_attribute("outer", vec![translate(10.0, 0.0, 0.0)]);
        scene.create("inner", "transform");
        scene.set_attribute("inner", vec![translate(1.0, 0.0, 0.0)]);
        scene.create("mesh", "mesh");
        scene.connect("mesh", None, "inner", "objects").unwrap();
        scene.connect("inner", None, "outer", "objects").unwrap();
        scene.connect("outer", None, ".root", "objects").unwrap();

        let m = scene.world_transform("mesh");
        assert_eq!(m[12], 11.0);
    }

    /// Order matters, and this is the test that catches composing the
    /// chain backwards. ɴsɪ is row-vector (RenderMan) convention, so a
    /// child's matrix applies before its parent's: scaling by 2 under a
    /// translation of 10 puts the origin at 10, whereas translating
    /// under a scale would put it at 20.
    #[test]
    fn child_transform_applies_before_parent() {
        let mut scene = Scene::default();
        scene.create("outer", "transform");
        scene.set_attribute("outer", vec![translate(10.0, 0.0, 0.0)]);
        scene.create("inner", "transform");
        scene.set_attribute("inner", vec![scale(2.0)]);
        scene.create("mesh", "mesh");
        scene.connect("mesh", None, "inner", "objects").unwrap();
        scene.connect("inner", None, "outer", "objects").unwrap();
        scene.connect("outer", None, ".root", "objects").unwrap();

        let m = scene.world_transform("mesh");
        assert_eq!(m[0], 2.0, "scale survives");
        assert_eq!(m[12], 10.0, "translation is not scaled");
    }

    /// A transform node's own matrix counts, not just its ancestors'.
    #[test]
    fn a_transforms_own_matrix_is_included() {
        let mut scene = Scene::default();
        scene.create("xf", "transform");
        scene.set_attribute("xf", vec![translate(5.0, 0.0, 0.0)]);
        scene.connect("xf", None, ".root", "objects").unwrap();
        assert_eq!(scene.world_transform("xf")[12], 5.0);
    }

    /// A cycle must not hang the resolver. ɴsɪ does not forbid one.
    #[test]
    fn a_cycle_terminates() {
        let mut scene = Scene::default();
        scene.create("a", "transform");
        scene.create("b", "transform");
        scene.connect("a", None, "b", "objects").unwrap();
        scene.connect("b", None, "a", "objects").unwrap();
        let _ = scene.world_transform("a");
    }
}

#[cfg(test)]
mod binding_tests {
    use crate::Scene;

    /// The canonical ɴsɪ shape: shader -> attributes -> geometry.
    fn scene_with_material() -> Scene {
        let mut scene = Scene::default();
        scene.create("mesh", "mesh");
        scene.create("attr", "attributes");
        scene.create("shader", "shader");
        scene
            .connect("attr", None, "mesh", "geometryattributes")
            .unwrap();
        scene
            .connect("shader", None, "attr", "surfaceshader")
            .unwrap();
        scene
    }

    #[test]
    fn dissolves_attributes_to_a_shader() {
        let scene = scene_with_material();
        let binding = scene.geometry_binding("mesh").expect("bound");
        assert_eq!(binding.attributes, "attr");
        assert_eq!(binding.surface_shader.as_deref(), Some("shader"));
    }

    #[test]
    fn unbound_geometry_has_no_binding() {
        let mut scene = Scene::default();
        scene.create("mesh", "mesh");
        assert!(scene.geometry_binding("mesh").is_none());
    }

    /// An attributes node with no shader still binds -- it may carry
    /// only visibility flags.
    #[test]
    fn attributes_without_a_shader_still_bind() {
        let mut scene = Scene::default();
        scene.create("mesh", "mesh");
        scene.create("attr", "attributes");
        scene
            .connect("attr", None, "mesh", "geometryattributes")
            .unwrap();
        let binding = scene.geometry_binding("mesh").expect("bound");
        assert_eq!(binding.attributes, "attr");
        assert!(binding.surface_shader.is_none());
    }

    /// One attributes node bound to several shapes must resolve for each
    /// of them. This is the fan-out the spec calls out.
    #[test]
    fn one_attributes_node_fans_out_to_every_shape() {
        let mut scene = Scene::default();
        scene.create("attr", "attributes");
        scene.create("shader", "shader");
        scene
            .connect("shader", None, "attr", "surfaceshader")
            .unwrap();
        for mesh in ["a", "b", "c"] {
            scene.create(mesh, "mesh");
            scene
                .connect("attr", None, mesh, "geometryattributes")
                .unwrap();
        }
        for mesh in ["a", "b", "c"] {
            let binding = scene.geometry_binding(mesh).expect("bound");
            assert_eq!(binding.surface_shader.as_deref(), Some("shader"));
        }
    }
}

#[cfg(test)]
mod output_tests {
    use crate::Scene;

    /// The canonical ɴsɪ output chain:
    /// driver -> layer -> screen -> camera.
    fn scene_with_output() -> Scene {
        let mut scene = Scene::default();
        scene.create("cam", "perspectivecamera");
        scene.create("scr", "screen");
        scene.create("beauty", "outputlayer");
        scene.create("drv", "outputdriver");
        scene.connect("scr", None, "cam", "screens").unwrap();
        scene
            .connect("beauty", None, "scr", "outputlayers")
            .unwrap();
        scene
            .connect("drv", None, "beauty", "outputdrivers")
            .unwrap();
        scene
    }

    #[test]
    fn resolves_the_whole_output_chain() {
        let scene = scene_with_output();
        let outputs = scene.render_outputs();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].camera, "cam");
        assert_eq!(outputs[0].screen, "scr");
        assert_eq!(outputs[0].layers.len(), 1);
        assert_eq!(outputs[0].layers[0].handle, "beauty");
        assert_eq!(outputs[0].layers[0].drivers, vec!["drv".to_string()]);
    }

    /// A screen with no layers is still a render output -- the camera
    /// and resolution are meaningful on their own.
    #[test]
    fn a_screen_without_layers_still_resolves() {
        let mut scene = Scene::default();
        scene.create("cam", "perspectivecamera");
        scene.create("scr", "screen");
        scene.connect("scr", None, "cam", "screens").unwrap();
        let outputs = scene.render_outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].layers.is_empty());
    }

    /// Several AOVs on one screen, in connection order.
    #[test]
    fn multiple_layers_keep_connection_order() {
        let mut scene = scene_with_output();
        scene.create("depth", "outputlayer");
        scene.connect("depth", None, "scr", "outputlayers").unwrap();
        let outputs = scene.render_outputs();
        let names: Vec<&str> = outputs[0]
            .layers
            .iter()
            .map(|l| l.handle.as_str())
            .collect();
        assert_eq!(names, vec!["beauty", "depth"]);
    }

    /// One layer fanned out to two drivers -- a file and a display.
    #[test]
    fn a_layer_may_have_several_drivers() {
        let mut scene = scene_with_output();
        scene.create("drv2", "outputdriver");
        scene
            .connect("drv2", None, "beauty", "outputdrivers")
            .unwrap();
        let outputs = scene.render_outputs();
        assert_eq!(outputs[0].layers[0].drivers, vec!["drv", "drv2"]);
    }

    #[test]
    fn no_screen_means_no_outputs() {
        let mut scene = Scene::default();
        scene.create("cam", "perspectivecamera");
        assert!(scene.render_outputs().is_empty());
    }
}

#[cfg(test)]
mod instance_tests {
    use crate::Scene;

    #[test]
    fn resolves_instance_source_models() {
        let mut scene = Scene::default();
        scene.create("inst", "instances");
        scene.create("proto_a", "mesh");
        scene.create("proto_b", "mesh");
        scene
            .connect("proto_a", None, "inst", "sourcemodels")
            .unwrap();
        scene
            .connect("proto_b", None, "inst", "sourcemodels")
            .unwrap();
        assert_eq!(scene.instance_sources("inst"), vec!["proto_a", "proto_b"]);
    }

    #[test]
    fn an_instances_node_with_no_sources_is_empty() {
        let mut scene = Scene::default();
        scene.create("inst", "instances");
        assert!(scene.instance_sources("inst").is_empty());
    }
}
