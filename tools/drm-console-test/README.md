# DRM Console Conflict Test

This is a manual test for the DRM/KMS and VT framebuffer-console ownership bug.
It verifies that a real DRM modeset switches the active VT console to graphics
mode, so kernel console output no longer overwrites the graphical scanout.

Build inside the Asterinas Docker container:

```sh
cd /root/asterinas
gcc -O2 -Wall -Wextra -o /tmp/drm_console_conflict \
    tools/drm-console-test/drm_console_conflict.c
```

Copy the binary into the ext2 image if you want to run it from the Asterinas
guest:

```sh
mkdir -p /tmp/aster-ext2
mount -o loop test/initramfs/build/ext2.img /tmp/aster-ext2
cp /tmp/drm_console_conflict /tmp/aster-ext2/
umount /tmp/aster-ext2
```

Run in the Asterinas guest:

```sh
mount /dev/vdb /mnt
/mnt/drm_console_conflict
```

Expected result:

- The program opens `/dev/dri/card0`, creates a dumb framebuffer, performs
  `DRM_IOCTL_MODE_SETCRTC`, draws a visible solid-color scanout, and triggers
  syscall-334 log probes.
- `KDGETMODE` should report `KD_GRAPHICS` after `MODE_SETCRTC`.
- The QEMU display should keep showing the solid-color DRM scanout. Kernel log text
  must not be drawn over the pattern while the DRM file owns the display.
- After the DRM file is closed or drops master, the console may return to
  `KD_TEXT`.

If a recording needs visible kernel `error` logs while keeping normal boot logs
quiet, add a temporary kernel-side probe for syscall 334 and rebuild the kernel.
Do not keep that probe in production changes.
