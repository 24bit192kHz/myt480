//!HOOK MAIN
//!BIND HOOKED
//!DESC memory bandwidth load: 32 incoherent reads of the 4K source per pixel
vec4 hook() {
    vec4 acc = vec4(0.0);
    for (int i = 0; i < 32; i++) {
        float k = float(i);
        vec2 q = fract(HOOKED_pos * vec2(17.0 + 3.0 * k, 23.0 + 5.0 * k) + vec2(k * 0.1337, k * 0.0731));
        acc += HOOKED_tex(q);
    }
    return acc / 32.0;
}
