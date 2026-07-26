// Profile node `mix_bsdf` -- GPU source of record (requirement R4).
void nsi_mix_bsdf(in NsiShadingContext ctx,
                  in NsiClosure a,
                  in NsiClosure b,
                  in float t,
                  out NsiClosure out_bsdf) {
    out_bsdf = nsi_closure_mix(a, b, t);
}
