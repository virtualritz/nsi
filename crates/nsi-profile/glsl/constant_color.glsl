// Profile node `constant_color` -- GPU source of record (requirement R4).
void nsi_constant_color(in NsiShadingContext ctx,
                        in vec3 value,
                        out vec3 out_color) {
    out_color = value;
}
