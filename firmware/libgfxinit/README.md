# libgfxinit

Patches for `3rdparty/libgfxinit` in the coreboot tree, on top of `BASE-COMMIT`.

They make the firmware set up the eDP link the way Linux's i915 does (DPLL0 at HBR,
link N = 0x80000), so i915 can take over the running display instead of turning it off
and on again at boot.

```sh
cd coreboot/3rdparty/libgfxinit
git checkout "$(cat BASE-COMMIT)"
git am patches/*.patch
```

Apply them before building. Do not edit the messages or dates of these patches: the
coreboot patches 0003, 0004 and 0006 refer to the resulting commits by hash.
