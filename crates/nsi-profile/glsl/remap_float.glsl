// Profile node `remap_float` -- GPU source of record (requirement R4).
void nsi_remap_float(in NsiShadingContext ctx,
                     in float in_float,
                     in float inlow,
                     in float inhigh,
                     in float outlow,
                     in float outhigh,
                     out float out_float) {
    float span = inhigh - inlow;
    float t = abs(span) > 1.0e-8 ? (in_float - inlow) / span : 0.0;
    out_float = outlow + t * (outhigh - outlow);
}
