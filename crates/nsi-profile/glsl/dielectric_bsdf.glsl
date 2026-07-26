// Profile node `dielectric_bsdf` -- GPU source of record (requirement R4).
//
// Emits the profile `microfacet` closure twice: one reflect lobe and one
// refract lobe, matching the single `dielectric_bsdf()` OSL closure, which
// carries both.
void nsi_dielectric_bsdf(in NsiShadingContext ctx,
                         in float ior,
                         in float roughness,
                         in vec3 transmission_color,
                         in vec3 shading_normal,
                         in vec3 tangent,
                         out NsiClosure out_bsdf) {
    float alpha = clamp(roughness, 0.0, 1.0);
    alpha = alpha * alpha;

    vec3 frame_n = normalize(shading_normal);
    vec3 frame_t = nsi_orthonormal_tangent(frame_n, tangent);

    NsiLobe reflect_lobe = nsi_lobe_neutral();
    reflect_lobe.kind = NSI_LOBE_DIELECTRIC;
    reflect_lobe.mode = NSI_MODE_REFLECT;
    reflect_lobe.weight = vec3(1.0);
    reflect_lobe.N = frame_n;
    reflect_lobe.U = frame_t;
    reflect_lobe.roughness = alpha;
    reflect_lobe.anisotropy = alpha;
    reflect_lobe.ior = max(ior, 1.0);

    NsiLobe refract_lobe = reflect_lobe;
    refract_lobe.mode = NSI_MODE_REFRACT;
    refract_lobe.weight = clamp(transmission_color, vec3(0.0), vec3(1.0));

    out_bsdf = nsi_closure_push(
        nsi_closure_push(nsi_closure_zero(), reflect_lobe), refract_lobe);
}
