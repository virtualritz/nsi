#version 460

// NSI shading profile v1 -- shared GLSL preamble.
//
// This is the first section of every assembled network module (requirement
// R4, resolved 2026-07-26: GLSL 4.60 is the GPU source of record; SPIR-V
// compilation is a backend step behind the `GpuEmitter` trait).
//
// It defines the closure aggregates that stand in for OSL's `closure color`
// on the GPU, the shading context, and the small helper library the node
// functions are written against.
//
// Node function ABI
// -----------------
//
//     void nsi_<node>(in NsiShadingContext ctx,
//                     in <input ports, in declaration order>,
//                     out <sole output port>);
//
// Every node function takes the shading context first, whether it reads it
// or not, so that the translator can emit call sites mechanically. String
// ports arrive as `int` enumerants (index into the port's allowed value
// list) or as texture indices, both resolved at translation time -- which is
// why changing a string parameter requires re-translation rather than a
// parameter-block write.
//
// Vulkan GLSL is assumed: descriptor sets are used, per the Vulkan-first
// decision of feature 001.

// ---------------------------------------------------------------------------
// Limits.
// ---------------------------------------------------------------------------

// Maximum number of scattering lobes a single closure value can carry.
const int NSI_LOBE_MAX = 8;

// Maximum number of distinct textures a translated network may reference.
const int NSI_TEXTURE_MAX = 16;

// ---------------------------------------------------------------------------
// Closure lobe kinds. These mirror the profile closure table one-to-one,
// except `emission` and `holdout`, which live on NsiSurface because they are
// not scattering.
// ---------------------------------------------------------------------------

const int NSI_LOBE_DIFFUSE = 0;     // closure `diffuse`, Oren-Nayar.
const int NSI_LOBE_CONDUCTOR = 1;   // closure `microfacet`, conductor Fresnel.
const int NSI_LOBE_DIELECTRIC = 2;  // closure `microfacet`, dielectric Fresnel.
const int NSI_LOBE_SHEEN = 3;       // closure `sheen`.
const int NSI_LOBE_TRANSPARENT = 4; // closure `transparent`.

// Microfacet lobe modes.
const int NSI_MODE_REFLECT = 0;
const int NSI_MODE_REFRACT = 1;

// `math_color` operations. The order is the order of the port's allowed
// value list, which is what the translator emits as the enumerant.
const int NSI_OP_ADD = 0;
const int NSI_OP_SUBTRACT = 1;
const int NSI_OP_MULTIPLY = 2;
const int NSI_OP_DIVIDE = 3;
const int NSI_OP_MIN = 4;
const int NSI_OP_MAX = 5;

// ---------------------------------------------------------------------------
// Aggregates.
// ---------------------------------------------------------------------------

// One scattering lobe. Unused fields carry their neutral value; the
// integrator dispatches on `kind`.
struct NsiLobe {
    int kind;
    int mode;
    vec3 weight;
    vec3 N;
    vec3 U;
    vec3 eta;
    vec3 k;
    float roughness;
    float anisotropy;
    float ior;
};

// A scattering closure: the GPU stand-in for a `Bsdf` port.
struct NsiClosure {
    int count;
    NsiLobe lobes[NSI_LOBE_MAX];
};

// A complete surface: the GPU stand-in for a `Surface` port, and the type of
// a network terminal.
struct NsiSurface {
    NsiClosure scatter;
    vec3 emission;
    vec3 opacity;
    float holdout;
};

// Everything a node function may read from the geometry.
struct NsiShadingContext {
    vec3 P;
    vec3 N;
    vec3 Ng;
    vec3 U;
    vec3 uv;
};

// ---------------------------------------------------------------------------
// Texture bindings. The translator assigns each distinct `image.filename` a
// stable index into this array; indices are literals at the call site and are
// therefore dynamically uniform.
// ---------------------------------------------------------------------------

layout(set = 0, binding = 1) uniform sampler2D nsi_textures[NSI_TEXTURE_MAX];

// ---------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------

NsiLobe nsi_lobe_neutral() {
    NsiLobe lobe;
    lobe.kind = NSI_LOBE_DIFFUSE;
    lobe.mode = NSI_MODE_REFLECT;
    lobe.weight = vec3(0.0);
    lobe.N = vec3(0.0, 0.0, 1.0);
    lobe.U = vec3(1.0, 0.0, 0.0);
    lobe.eta = vec3(1.0);
    lobe.k = vec3(0.0);
    lobe.roughness = 0.0;
    lobe.anisotropy = 0.0;
    lobe.ior = 1.0;
    return lobe;
}

NsiClosure nsi_closure_zero() {
    NsiClosure closure_value;
    closure_value.count = 0;
    for (int i = 0; i < NSI_LOBE_MAX; ++i) {
        closure_value.lobes[i] = nsi_lobe_neutral();
    }
    return closure_value;
}

NsiClosure nsi_closure_push(NsiClosure closure_value, NsiLobe lobe) {
    if (closure_value.count < NSI_LOBE_MAX) {
        closure_value.lobes[closure_value.count] = lobe;
        closure_value.count += 1;
    }
    return closure_value;
}

// Scales every lobe weight. This is closure multiplication by a color, the
// GPU equivalent of `weight * closure` in OSL.
NsiClosure nsi_closure_scale(NsiClosure closure_value, vec3 weight) {
    for (int i = 0; i < NSI_LOBE_MAX; ++i) {
        if (i < closure_value.count) {
            closure_value.lobes[i].weight *= weight;
        }
    }
    return closure_value;
}

// Concatenates two closures. Lobes past NSI_LOBE_MAX are dropped rather than
// merged: silently merging would change appearance without saying so.
NsiClosure nsi_closure_add(NsiClosure a, NsiClosure b) {
    NsiClosure result = a;
    for (int i = 0; i < NSI_LOBE_MAX; ++i) {
        if (i < b.count) {
            result = nsi_closure_push(result, b.lobes[i]);
        }
    }
    return result;
}

NsiClosure nsi_closure_mix(NsiClosure a, NsiClosure b, float t) {
    float w = clamp(t, 0.0, 1.0);
    return nsi_closure_add(nsi_closure_scale(a, vec3(1.0 - w)),
                           nsi_closure_scale(b, vec3(w)));
}

NsiSurface nsi_surface_zero() {
    NsiSurface surface_value;
    surface_value.scatter = nsi_closure_zero();
    surface_value.emission = vec3(0.0);
    surface_value.opacity = vec3(1.0);
    surface_value.holdout = 0.0;
    return surface_value;
}

vec3 nsi_math(vec3 a, vec3 b, int op) {
    vec3 result = a + b;
    if (op == NSI_OP_SUBTRACT) {
        result = a - b;
    } else if (op == NSI_OP_MULTIPLY) {
        result = a * b;
    } else if (op == NSI_OP_DIVIDE) {
        result = vec3(b.x != 0.0 ? a.x / b.x : 0.0,
                      b.y != 0.0 ? a.y / b.y : 0.0,
                      b.z != 0.0 ? a.z / b.z : 0.0);
    } else if (op == NSI_OP_MIN) {
        result = min(a, b);
    } else if (op == NSI_OP_MAX) {
        result = max(a, b);
    }
    return result;
}

// Gram-Schmidt orthonormalisation of a tangent against a normal. Shared by
// every anisotropic node so that both targets build the same frame.
vec3 nsi_orthonormal_tangent(vec3 n, vec3 t) {
    vec3 projected = t - n * dot(n, t);
    float len = length(projected);
    return len > 1.0e-8 ? projected / len
                        : normalize(cross(n, vec3(0.0, 0.0, 1.0)));
}
