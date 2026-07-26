// Profile node `sheen_bsdf` -- GPU source of record (requirement R4).
//
// Emits the profile `sheen` closure.
void nsi_sheen_bsdf(in NsiShadingContext ctx,
                    in vec3 base_color,
                    in float roughness,
                    in vec3 shading_normal,
                    out NsiClosure out_bsdf) {
    NsiLobe lobe = nsi_lobe_neutral();
    lobe.kind = NSI_LOBE_SHEEN;
    lobe.weight = clamp(base_color, vec3(0.0), vec3(1.0));
    lobe.N = normalize(shading_normal);
    lobe.roughness = clamp(roughness, 0.0, 1.0);

    out_bsdf = nsi_closure_push(nsi_closure_zero(), lobe);
}
