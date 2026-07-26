// Profile node `holdout_surface` -- GPU source of record (requirement R4).
//
// Emits the profile `holdout` closure.
void nsi_holdout_surface(in NsiShadingContext ctx, out NsiSurface out_surface) {
    out_surface = nsi_surface_zero();
    out_surface.holdout = 1.0;
}
