// Profile node `uv` -- GPU source of record (requirement R4).
//
// Only the primary texture coordinate set exists in v1; see `osl/uv.osl`.
void nsi_uv(in NsiShadingContext ctx, out vec3 out_vector) {
    out_vector = ctx.uv;
}
