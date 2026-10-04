# Build and test environment for the T480 early init (musl, static binaries, swtpm).
FROM alpine:3.22
RUN apk add --no-cache build-base linux-headers pkgconf curl xz \
    lvm2-dev device-mapper-static lvm2-static util-linux-dev util-linux-static \
    json-c-dev popt-dev popt-static argon2-dev argon2-static \
    swtpm tpm2-tools tpm2-tss-tcti-swtpm keyutils e2fsprogs e2fsprogs-static e2fsprogs-extra sfdisk \
    cmake samurai bash xxd
# json-c has no static library package
RUN cd /tmp && curl -sL https://s3.amazonaws.com/json-c_releases/releases/json-c-0.18.tar.gz | tar xz && \
    cmake -S json-c-0.18 -B jb -G Ninja -DBUILD_SHARED_LIBS=OFF -DBUILD_STATIC_LIBS=ON -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=MinSizeRel -DDISABLE_WERROR=ON >/dev/null && \
    ninja -C jb install >/dev/null && rm -rf json-c-0.18 jb
ARG CS=2.8.1
RUN cd /tmp && curl -sL https://www.kernel.org/pub/linux/utils/cryptsetup/v2.8/cryptsetup-${CS}.tar.xz | tar xJ && cd cryptsetup-${CS} && \
    ./configure --enable-static-cryptsetup --disable-shared --enable-static --disable-udev --disable-veritysetup \
      --disable-integritysetup --disable-ssh-token --disable-external-tokens --disable-asciidoc --disable-nls \
      --disable-blkid --disable-keyring --disable-luks2-reencryption --with-crypto_backend=kernel \
      CFLAGS="-Os" >/dev/null && \
    make -j"$(nproc)" >/dev/null && strip -o /usr/local/bin/cryptsetup.static cryptsetup.static && ls -la /usr/local/bin/cryptsetup.static
RUN apk add --no-cache util-linux-misc cpio libaio-dev zlib-dev zlib-static
