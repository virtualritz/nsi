// Profile node `math_color` -- GPU source of record (requirement R4).
//
// `op` arrives as the index of the selected constant in the port's allowed
// value list, matching the NSI_OP_* constants in `common.glsl`.
void nsi_math_color(in NsiShadingContext ctx,
                    in vec3 a,
                    in vec3 b,
                    in int op,
                    out vec3 out_color) {
    out_color = nsi_math(a, b, op);
}
