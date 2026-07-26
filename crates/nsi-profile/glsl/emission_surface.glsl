// Profile node `emission_surface` -- GPU source of record (requirement R4).
//
// Emits the profile `emission` closure. The weight is radiance in W/sr/m^2
// and is not normalised.
void nsi_emission_surface(in NsiShadingContext ctx,
                          in vec3 base_color,
                          in float intensity,
                          out NsiSurface out_surface) {
    out_surface = nsi_surface_zero();
    out_surface.emission = max(intensity, 0.0) * base_color;
}
