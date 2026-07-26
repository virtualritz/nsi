// Profile node `image` -- GPU source of record (requirement R4).
//
// `filename` arrives as the texture index the translator assigned to the
// file, which is why changing the file requires re-translation. The `v`
// coordinate is not flipped here: the sampler convention already matches the
// flip applied by `osl/image.osl`.
void nsi_image(in NsiShadingContext ctx,
               in int filename,
               in vec3 uv,
               in vec3 default_color,
               out vec3 out_color) {
    if (filename < 0 || filename >= NSI_TEXTURE_MAX) {
        out_color = default_color;
    } else {
        out_color = texture(nsi_textures[filename], uv.xy).rgb;
    }
}
