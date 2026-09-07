//! The camera/screen/layer/driver chain, which is what a renderer
//! reads to know where pixels go.

use super::*;

impl Scene {
    /// Resolve ɴsɪ's output chain into what a renderer actually needs.
    ///
    /// ɴsɪ spreads this over four nodes and three connection classes  --
    /// `outputdriver -> outputlayer -> screen -> camera` -- where both
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
        self.edges()
            .filter(|edge| edge.kind == EdgeKind::Screen)
            .map(|screen_edge| {
                let screen = &screen_edge.from;
                let mut layers: Vec<OutputLayer> = self
                    .edges_to_attribute(
                        screen,
                        EdgeKind::OutputLayer.to_attribute(),
                    )
                    .filter(|edge| edge.kind == EdgeKind::OutputLayer)
                    .map(|layer_edge| self.output_layer(layer_edge.from()))
                    .collect();

                // ɴsɪ: "Layers with the lowest sortkey attribute
                // appear first" among the layers on one driver. A
                // stable sort keeps connection order for the layers
                // that set no key, and `None` sorts before `Some` --
                // an unkeyed layer is not last, it is unordered, and
                // connection order is the only thing left to honour.
                layers.sort_by_key(|layer| layer.sort_key);

                RenderOutput {
                    camera: screen_edge.to().to_string(),
                    screen: screen.to_string(),
                    layers,
                }
            })
            .collect()
    }
}

impl Scene {
    /// One `outputlayer` node, with the specification's defaults
    /// applied to every attribute it does not set.
    fn output_layer(&self, handle: &str) -> OutputLayer {
        let node = self.node(handle);
        let string = |name: &str| {
            node.and_then(|node| node.string(name)).map(str::to_string)
        };
        let or = |name: &str, default: &str| {
            string(name).unwrap_or_else(|| default.to_string())
        };

        OutputLayer {
            handle: handle.to_string(),
            variable_name: string(VARIABLE_NAME),
            variable_source: or(VARIABLE_SOURCE, "shader"),
            layer_name: string(LAYER_NAME),
            layer_type: or(LAYER_TYPE, "color"),
            scalar_format: or(SCALAR_FORMAT, "uint8"),
            with_alpha: node
                .and_then(|node| node.flag(WITH_ALPHA))
                .unwrap_or(false),
            dithering: node
                .and_then(|node| node.flag(DITHERING))
                .unwrap_or(false),
            filter: or(FILTER, "blackman-harris"),
            filter_width: node
                .and_then(|node| node.f64(FILTER_WIDTH))
                .unwrap_or(3.0),
            color_profile: string(COLOR_PROFILE),
            sort_key: node.and_then(|node| node.i32(SORT_KEY)),
            drivers: self
                .edges_to_attribute(
                    handle,
                    EdgeKind::OutputDriver.to_attribute(),
                )
                .filter(|edge| edge.kind == EdgeKind::OutputDriver)
                .map(|edge| edge.from().to_string())
                .collect(),
        }
    }
}
