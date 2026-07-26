//! Network translation -- [`Network`] to [`NetworkModule`].
//!
//! Translation assembles one GLSL 4.60 module from the per-node GLSL
//! functions, computes the [`ParameterBlockLayout`] and reports which
//! closures the network can emit. Compiling that module to SPIR-V is a
//! backend step behind [`GpuEmitter`](crate::emit::GpuEmitter) (requirement
//! R4, resolved 2026-07-26), so this crate depends on no shader compiler.
//!
//! # Validation Is Not Optional
//!
//! [`translate`] runs [`validate`] itself and refuses a network with any
//! violation, returning [`Error::NotConforming`] with the full report. There
//! is no way to translate an out-of-profile network through this API
//! (`contracts/profile-conformance.md`, failure modes).
//!
//! # Assembled Module Layout
//!
//! 1. The shared preamble, `glsl/common.glsl`.
//! 2. The `NsiParameterBlock` storage block, if the network has any
//!    animatable parameter, with an explicit `layout(offset = ...)` on every
//!    member so the GPU-side layout cannot drift from
//!    [`ParameterBlockLayout`].
//! 3. One function per distinct profile node the network uses, in profile
//!    table order.
//! 4. The entry point, [`ENTRY_POINT`], which declares one local per node
//!    output and calls the node functions in topological order.
//!
//! The output is deterministic: the same network always yields byte-for-byte
//! the same module.
use crate::{
    error::Error,
    network::{Network, ParamValue},
    node::{NodeDef, Port, PortDefault, PortType},
    parameter_block::ParameterBlockLayout,
    registry::Registry,
    v1::GLSL_COMMON,
    validate::validate,
    version::{Version, parse_scheme},
};

/// The entry point of every assembled module.
pub const ENTRY_POINT: &str = "nsi_network_main";

/// A texture the module samples, and the index it was assigned.
///
/// Texture indices are baked into the module, which is why changing an
/// [`image`](crate::v1::IMAGE) file name is a re-translation rather than a
/// parameter update.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureBinding {
    /// Index into the module's `nsi_textures` array.
    pub index: u32,
    /// The file name the scene set.
    pub filename: String,
}

/// The result of translating one network.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetworkModule {
    glsl_source: String,
    parameter_block: ParameterBlockLayout,
    initial_parameters: Vec<u8>,
    closure_signature: Vec<String>,
    textures: Vec<TextureBinding>,
    profile_version: Version,
    terminal_handle: String,
}

impl NetworkModule {
    /// The closures this network can emit, in profile table order.
    #[must_use]
    pub fn closure_signature(&self) -> &[String] {
        &self.closure_signature
    }

    /// The entry point name.
    #[must_use]
    pub const fn entry_point(&self) -> &'static str {
        ENTRY_POINT
    }

    /// The assembled GLSL 4.60 module.
    #[must_use]
    pub fn glsl_source(&self) -> &str {
        &self.glsl_source
    }

    /// A parameter buffer filled with the values the scene set, and the port
    /// defaults everywhere else.
    #[must_use]
    pub fn initial_parameters(&self) -> &[u8] {
        &self.initial_parameters
    }

    /// The animatable-parameter layout (requirement R6).
    #[must_use]
    pub const fn parameter_block(&self) -> &ParameterBlockLayout {
        &self.parameter_block
    }

    /// The profile version this module was translated against.
    #[must_use]
    pub const fn profile_version(&self) -> &Version {
        &self.profile_version
    }

    /// The handle of the network's terminal `shader` node.
    #[must_use]
    pub fn terminal_handle(&self) -> &str {
        &self.terminal_handle
    }

    /// The textures the module samples, by assigned index.
    #[must_use]
    pub fn textures(&self) -> &[TextureBinding] {
        &self.textures
    }
}

/// Translates a network against the registry's newest profile.
///
/// # Errors
///
/// See [`translate_with_version`].
pub fn translate(
    network: &Network,
    registry: &Registry,
) -> Result<NetworkModule, Error> {
    translate_with_version(network, registry, &registry.latest_version())
}

/// Translates a network against one profile version.
///
/// # Errors
///
/// - [`Error::NotConforming`] if validation reports anything at all,
///   including an unregistered `version`. Cycles are reported this way too.
/// - [`Error::MissingTerminal`] or [`Error::AmbiguousTerminal`] if the
///   network does not have exactly one unconnected `Surface` output.
///
/// # Panics
///
/// Never: every node is resolved by the validation pass that precedes
/// translation, so the resolution below cannot fail.
pub fn translate_with_version(
    network: &Network,
    registry: &Registry,
    version: &Version,
) -> Result<NetworkModule, Error> {
    let report = validate(network, registry, version);

    match registry.profile(version).filter(|_| report.is_conforming()) {
        None => Err(Error::NotConforming {
            version: version.clone(),
            report,
        }),
        Some(profile) => {
            let order = network.topological_order()?;

            let definitions: Vec<&'static NodeDef> = order
                .iter()
                .map(|&index| {
                    let node = &network.nodes()[index];

                    parse_scheme(&node.shaderfilename)
                        .ok()
                        .and_then(|reference| profile.node(reference.node))
                        .expect("validation resolved every node")
                })
                .collect();

            let assembly = Assembly {
                network,
                order: &order,
                definitions: &definitions,
                textures: texture_bindings(network, &order, &definitions),
            };

            let terminal = assembly.terminal_index()?;
            let parameter_block = assembly.layout();
            let initial_parameters =
                assembly.initial_parameters(&parameter_block);

            let closure_signature = profile
                .closures()
                .iter()
                .map(|closure| closure.name)
                .filter(|name| {
                    definitions
                        .iter()
                        .any(|definition| definition.closures.contains(name))
                })
                .map(ToString::to_string)
                .collect();

            Ok(NetworkModule {
                glsl_source: assembly.assemble(
                    profile.nodes(),
                    &parameter_block,
                    terminal,
                ),
                parameter_block,
                initial_parameters,
                closure_signature,
                terminal_handle: network.nodes()[order[terminal]]
                    .handle
                    .clone(),
                textures: assembly.textures,
                profile_version: version.clone(),
            })
        }
    }
}

/// Assigns a stable texture index to every distinct resource string, in
/// topological node order.
fn texture_bindings(
    network: &Network,
    order: &[usize],
    definitions: &[&'static NodeDef],
) -> Vec<TextureBinding> {
    let mut textures: Vec<TextureBinding> = Vec::new();

    order
        .iter()
        .zip(definitions)
        .for_each(|(&index, definition)| {
            let node = &network.nodes()[index];

            definition
                .inputs
                .iter()
                .filter(|port| {
                    port.ty == PortType::String && port.allowed.is_empty()
                })
                .filter_map(|port| resource_value(node.param(port.name), port))
                .filter(|filename| !filename.is_empty())
                .for_each(|filename| {
                    if !textures
                        .iter()
                        .any(|texture| texture.filename == filename)
                    {
                        textures.push(TextureBinding {
                            index: u32::try_from(textures.len())
                                .expect("texture count fits in u32"),
                            filename,
                        });
                    }
                });
        });

    textures
}

/// The string value of a resource port -- the scene's value, else the port
/// default.
fn resource_value(value: Option<&ParamValue>, port: &Port) -> Option<String> {
    match (value, port.default) {
        (Some(ParamValue::String(text)), _) => Some(text.clone()),
        (_, Some(PortDefault::String(text))) => Some(text.to_string()),
        _ => None,
    }
}

/// A port's literal default as a [`ParamValue`].
fn default_value(port: &Port) -> Option<ParamValue> {
    match (port.default, port.ty) {
        (Some(PortDefault::Float(scalar)), _) => {
            Some(ParamValue::Float(scalar))
        }
        (Some(PortDefault::Int(scalar)), _) => Some(ParamValue::Int(scalar)),
        (Some(PortDefault::Triple(triple)), PortType::Color) => {
            Some(ParamValue::Color(triple))
        }
        (Some(PortDefault::Triple(triple)), PortType::Vector) => {
            Some(ParamValue::Vector(triple))
        }
        (Some(PortDefault::Triple(triple)), PortType::Normal) => {
            Some(ParamValue::Normal(triple))
        }
        (Some(PortDefault::Triple(triple)), PortType::Point) => {
            Some(ParamValue::Point(triple))
        }
        _ => None,
    }
}

/// Turns a handle into a GLSL-safe identifier fragment.
fn sanitize(handle: &str) -> String {
    handle
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

/// The GLSL member name of a parameter-block field.
fn field_name(position: usize, handle: &str, param: &str) -> String {
    format!("p{position}_{}_{param}", sanitize(handle))
}

/// The GLSL local holding a node's output.
fn local_name(position: usize, handle: &str, port: &str) -> String {
    format!("n{position}_{}_{port}", sanitize(handle))
}

/// A horizontal rule in the assembled source.
fn rule(title: &str) -> String {
    let bar = "-".repeat(72);

    format!("\n// {bar}\n// {title}\n// {bar}\n")
}

/// Everything the assembler needs about one already-validated network.
struct Assembly<'a> {
    network: &'a Network,
    order: &'a [usize],
    definitions: &'a [&'static NodeDef],
    textures: Vec<TextureBinding>,
}

impl Assembly<'_> {
    /// Assembles the module source.
    fn assemble(
        &self,
        table: &'static [NodeDef],
        parameter_block: &ParameterBlockLayout,
        terminal: usize,
    ) -> String {
        let mut source = String::from(GLSL_COMMON);

        source.push_str(&rule(
            "Network parameter block. Offsets are authoritative and match the\n\
             // `ParameterBlockLayout` this module was translated with.",
        ));

        if parameter_block.fields().is_empty() {
            source
                .push_str("\n// This network has no animatable parameters.\n");
        } else {
            source.push_str(
                "\nlayout(std430, set = 0, binding = 0) readonly buffer \
                 NsiParameterBlock {\n",
            );

            parameter_block.fields().iter().enumerate().for_each(
                |(position, field)| {
                    source.push_str(&format!(
                        "    layout(offset = {}) {} {};\n",
                        field.offset,
                        field.ty.glsl_type(),
                        field_name(position, &field.node_handle, &field.param)
                    ));
                },
            );

            source.push_str("} nsi_params;\n");
        }

        if !self.textures.is_empty() {
            source.push_str("\n// Texture bindings:\n");

            self.textures.iter().for_each(|texture| {
                source.push_str(&format!(
                    "//   {} -> `{}`\n",
                    texture.index, texture.filename
                ));
            });
        }

        source.push_str(&rule("Node functions."));

        table
            .iter()
            .filter(|node| {
                self.definitions
                    .iter()
                    .any(|definition| definition.name == node.name)
            })
            .for_each(|node| {
                source.push('\n');
                source.push_str(node.glsl_source.trim_end());
                source.push('\n');
            });

        source.push_str(&rule("Entry point."));
        source.push_str(&format!(
            "\nvoid {ENTRY_POINT}(in NsiShadingContext ctx, out NsiSurface out_surface) {{\n"
        ));

        let mut field_position = 0;

        self.order
            .iter()
            .zip(self.definitions)
            .enumerate()
            .for_each(|(position, (&index, definition))| {
                let node = &self.network.nodes()[index];
                let output = definition.sole_output();
                let local = local_name(position, &node.handle, output.name);

                let arguments = definition
                    .inputs
                    .iter()
                    .map(|port| {
                        self.input_expression(
                            &node.handle,
                            node.param(port.name),
                            port,
                            &mut field_position,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                source.push_str(&format!(
                    "    {} {local};\n    nsi_{}(ctx",
                    output.ty.glsl_type(),
                    definition.name
                ));

                if !arguments.is_empty() {
                    source.push_str(&format!(", {arguments}"));
                }

                source.push_str(&format!(", {local});\n\n"));
            });

        let terminal_node = &self.network.nodes()[self.order[terminal]];

        source.push_str(&format!(
            "    out_surface = {};\n}}\n",
            local_name(
                terminal,
                &terminal_node.handle,
                self.definitions[terminal].sole_output().name
            )
        ));

        source
    }

    /// Fills a parameter buffer with scene values, falling back to the port
    /// defaults -- which is not a silent fallback but the documented meaning
    /// of "the scene did not set this parameter".
    fn initial_parameters(
        &self,
        parameter_block: &ParameterBlockLayout,
    ) -> Vec<u8> {
        let mut buffer = parameter_block.zeroed_buffer();

        self.order.iter().zip(self.definitions).for_each(
            |(&index, definition)| {
                let node = &self.network.nodes()[index];

                definition
                    .inputs
                    .iter()
                    .filter(|port| self.is_block_field(&node.handle, port))
                    .for_each(|port| {
                        if let Some(value) = node
                            .param(port.name)
                            .cloned()
                            .or_else(|| default_value(port))
                        {
                            parameter_block
                                .write_param(
                                    &mut buffer,
                                    &node.handle,
                                    port.name,
                                    &value,
                                )
                                .expect(
                                    "layout and port agree by construction",
                                );
                        }
                    });
            },
        );

        buffer
    }

    /// The GLSL expression feeding one input port at a call site.
    fn input_expression(
        &self,
        handle: &str,
        value: Option<&ParamValue>,
        port: &Port,
        field_position: &mut usize,
    ) -> String {
        match self.network.connection_into(handle, port.name) {
            Some(connection) => self
                .order
                .iter()
                .position(|&index| {
                    self.network.nodes()[index].handle == connection.from_handle
                })
                .map_or_else(
                    || format!("{}(0)", port.ty.glsl_type()),
                    |position| {
                        local_name(
                            position,
                            &connection.from_handle,
                            self.definitions[position].sole_output().name,
                        )
                    },
                ),
            None if port.is_block_eligible() => {
                let name = field_name(*field_position, handle, port.name);
                *field_position += 1;

                format!("nsi_params.{name}")
            }
            None => match (port.ty, port.default) {
                (PortType::Bsdf, _) => "nsi_closure_zero()".to_string(),
                (PortType::Surface, _) => "nsi_surface_zero()".to_string(),
                (PortType::String, _) => {
                    self.string_literal(value, port).to_string()
                }
                (_, Some(PortDefault::Global(global))) => {
                    global.glsl_expr().to_string()
                }
                _ => format!("{}(0)", port.ty.glsl_type()),
            },
        }
    }

    /// Whether a port contributes a parameter-block field on this node.
    fn is_block_field(&self, handle: &str, port: &Port) -> bool {
        port.is_block_eligible()
            && self.network.connection_into(handle, port.name).is_none()
    }

    /// Computes the parameter-block layout for the network.
    fn layout(&self) -> ParameterBlockLayout {
        ParameterBlockLayout::new(
            self.order
                .iter()
                .zip(self.definitions)
                .flat_map(|(&index, definition)| {
                    let handle = &self.network.nodes()[index].handle;

                    definition
                        .inputs
                        .iter()
                        .filter(move |port| self.is_block_field(handle, port))
                        .map(move |port| {
                            (handle.clone(), port.name.to_string(), port.ty)
                        })
                })
                .collect::<Vec<_>>(),
        )
    }

    /// The `int` a string port is baked down to: an enumerant index, a
    /// texture index, or `-1` for "no resource".
    fn string_literal(&self, value: Option<&ParamValue>, port: &Port) -> i64 {
        let text = resource_value(value, port).unwrap_or_default();

        if port.allowed.is_empty() {
            self.textures
                .iter()
                .find(|texture| texture.filename == text)
                .map_or(-1, |texture| i64::from(texture.index))
        } else {
            port.allowed
                .iter()
                .position(|allowed| *allowed == text)
                .map_or(0, |position| {
                    i64::try_from(position).expect("enumerant index fits")
                })
        }
    }

    /// The position *within the topological order* of the terminal node.
    fn terminal_index(&self) -> Result<usize, Error> {
        let terminals: Vec<usize> = self
            .order
            .iter()
            .zip(self.definitions)
            .enumerate()
            .filter_map(|(position, (&index, definition))| {
                let output = definition.sole_output();

                (output.ty == PortType::Surface
                    && !self.network.is_output_connected(
                        &self.network.nodes()[index].handle,
                        output.name,
                    ))
                .then_some(position)
            })
            .collect();

        match terminals.as_slice() {
            [] => Err(Error::MissingTerminal),
            [only] => Ok(*only),
            _ => Err(Error::AmbiguousTerminal {
                handles: terminals
                    .iter()
                    .map(|&position| {
                        self.network.nodes()[self.order[position]]
                            .handle
                            .clone()
                    })
                    .collect(),
            }),
        }
    }
}
