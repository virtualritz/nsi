// Profile node `add_bsdf` -- GPU source of record (requirement R4).
//
// The sum is deliberately not renormalised; see `osl/add_bsdf.osl`.
void nsi_add_bsdf(in NsiShadingContext ctx,
                  in NsiClosure a,
                  in NsiClosure b,
                  out NsiClosure out_bsdf) {
    out_bsdf = nsi_closure_add(a, b);
}
