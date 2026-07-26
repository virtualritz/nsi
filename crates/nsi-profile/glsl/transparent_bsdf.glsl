// Profile node `transparent_bsdf` -- GPU source of record (requirement R4).
//
// Emits the profile `transparent` closure.
void nsi_transparent_bsdf(in NsiShadingContext ctx,
                          in vec3 base_color,
                          out NsiClosure out_bsdf) {
    NsiLobe lobe = nsi_lobe_neutral();
    lobe.kind = NSI_LOBE_TRANSPARENT;
    lobe.weight = clamp(base_color, vec3(0.0), vec3(1.0));

    out_bsdf = nsi_closure_push(nsi_closure_zero(), lobe);
}
