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
                let layers = self
                    .edges_to_attr(screen, EdgeKind::OutputLayer.to_attr())
                    .filter(|edge| edge.kind == EdgeKind::OutputLayer)
                    .map(|layer_edge| OutputLayer {
                        handle: layer_edge.from().to_string(),
                        drivers: self
                            .edges_to_attr(
                                &layer_edge.from,
                                EdgeKind::OutputDriver.to_attr(),
                            )
                            .filter(|edge| edge.kind == EdgeKind::OutputDriver)
                            .map(|edge| edge.from().to_string())
                            .collect(),
                    })
                    .collect();

                RenderOutput {
                    camera: screen_edge.to().to_string(),
                    screen: screen.to_string(),
                    layers,
                }
            })
            .collect()
    }
}
