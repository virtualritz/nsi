// Profile node `normal_map` -- GPU source of record (requirement R4).
void nsi_normal_map(in NsiShadingContext ctx,
                    in vec3 in_color,
                    in float strength,
                    in vec3 shading_normal,
                    in vec3 tangent,
                    out vec3 out_normal) {
    vec3 frame_n = normalize(shading_normal);
    vec3 frame_t = nsi_orthonormal_tangent(frame_n, tangent);
    vec3 frame_b = cross(frame_n, frame_t);

    vec3 encoded = in_color * 2.0 - 1.0;
    vec3 decoded = frame_t * (encoded.x * strength)
                 + frame_b * (encoded.y * strength)
                 + frame_n * encoded.z;

    out_normal = normalize(decoded);
}
