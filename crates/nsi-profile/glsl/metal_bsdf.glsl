// Profile node `metal_bsdf` -- GPU source of record (requirement R4).
//
// Emits the profile `microfacet` closure with conductor Fresnel. The
// Gulbrandsen (2014) artistic mapping below is line-for-line the one in
// `osl/metal_bsdf.osl`; the two must not drift.
void nsi_metal_bsdf(in NsiShadingContext ctx,
                    in vec3 base_color,
                    in vec3 edge_color,
                    in float roughness,
                    in float anisotropy,
                    in vec3 shading_normal,
                    in vec3 tangent,
                    out NsiClosure out_bsdf) {
    vec3 r = clamp(base_color, vec3(0.0), vec3(0.99));
    vec3 g = clamp(edge_color, vec3(0.0), vec3(1.0));
    vec3 root_r = sqrt(r);

    vec3 eta_min = (vec3(1.0) - r) / (vec3(1.0) + r);
    vec3 eta_max = (vec3(1.0) + root_r) / (vec3(1.0) - root_r);
    vec3 eta = mix(eta_max, eta_min, g);

    vec3 eta_plus = eta + vec3(1.0);
    vec3 eta_minus = eta - vec3(1.0);
    vec3 k_squared = (eta_plus * eta_plus * r - eta_minus * eta_minus)
                   / (vec3(1.0) - r);

    float alpha = clamp(roughness, 0.001, 1.0);
    alpha = alpha * alpha;
    float stretch = 1.0 - clamp(anisotropy, 0.0, 1.0) * 0.9;

    vec3 frame_n = normalize(shading_normal);

    NsiLobe lobe = nsi_lobe_neutral();
    lobe.kind = NSI_LOBE_CONDUCTOR;
    lobe.mode = NSI_MODE_REFLECT;
    lobe.weight = vec3(1.0);
    lobe.N = frame_n;
    lobe.U = nsi_orthonormal_tangent(frame_n, tangent);
    lobe.eta = eta;
    lobe.k = sqrt(max(k_squared, vec3(0.0)));
    lobe.roughness = alpha / stretch;
    lobe.anisotropy = alpha * stretch;

    out_bsdf = nsi_closure_push(nsi_closure_zero(), lobe);
}
