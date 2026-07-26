// Profile node `surface` -- GPU source of record (requirement R4).
//
// The network terminal. Opacity below one adds the profile `transparent`
// closure, matching the `(1 - coverage) * transparent()` term in
// `osl/surface.osl`.
void nsi_surface(in NsiShadingContext ctx,
                 in NsiClosure bsdf,
                 in NsiSurface emissive,
                 in vec3 opacity,
                 out NsiSurface out_surface) {
    vec3 coverage = clamp(opacity, vec3(0.0), vec3(1.0));

    NsiClosure scatter = nsi_closure_scale(bsdf, coverage);

    if (any(lessThan(coverage, vec3(1.0)))) {
        NsiLobe cutout = nsi_lobe_neutral();
        cutout.kind = NSI_LOBE_TRANSPARENT;
        cutout.weight = vec3(1.0) - coverage;
        scatter = nsi_closure_push(scatter, cutout);
    }

    out_surface = emissive;
    out_surface.scatter = nsi_closure_add(scatter, emissive.scatter);
    out_surface.emission = emissive.emission * coverage;
    out_surface.opacity = coverage;
}
