// Profile node `constant_float` -- GPU source of record (requirement R4).
void nsi_constant_float(in NsiShadingContext ctx,
                        in float value,
                        out float out_float) {
    out_float = value;
}
