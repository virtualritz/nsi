// Profile node `mix_color` -- GPU source of record (requirement R4).
void nsi_mix_color(in NsiShadingContext ctx,
                   in vec3 a,
                   in vec3 b,
                   in float t,
                   out vec3 out_color) {
    out_color = mix(a, b, clamp(t, 0.0, 1.0));
}
