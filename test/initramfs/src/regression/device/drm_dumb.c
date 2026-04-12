// SPDX-License-Identifier: MPL-2.0

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <unistd.h>

#include "../common/test.h"

#define DRM_DEVICE "/dev/dri/card0"
#define TEST_WIDTH 64
#define TEST_HEIGHT 64
#define TEST_BPP 32

/* DRM ioctl numbers - must match kernel definitions */
#define DRM_IOCTL_BASE 0x64
#define DRM_IOCTL_MODE_CREATE_DUMB _IOWR(DRM_IOCTL_BASE, 0x02, struct drm_mode_create_dumb)
#define DRM_IOCTL_MODE_MAP_DUMB _IOWR(DRM_IOCTL_BASE, 0x03, struct drm_mode_map_dumb)
#define DRM_IOCTL_MODE_DESTROY_DUMB _IOW(DRM_IOCTL_BASE, 0x04, struct drm_mode_destroy_dumb)

/* DRM structures - must match kernel definitions */
struct drm_mode_create_dumb {
    uint32_t height;
    uint32_t width;
    uint32_t bpp;
    uint32_t flags;
    uint32_t handle;
    uint32_t pitch;
    uint64_t size;
};

struct drm_mode_map_dumb {
    uint32_t handle;
    uint32_t pad;
    uint64_t offset;
};

struct drm_mode_destroy_dumb {
    uint32_t handle;
};

static int drm_fd = -1;
static uint32_t dumb_handle = 0;
static uint64_t dumb_offset = 0;
static uint8_t *mapped_addr = NULL;
static size_t mapped_size = 0;

FN_SETUP(open_drm_device)
{
	drm_fd = open(DRM_DEVICE, O_RDWR);
	if (drm_fd < 0) {
		if (errno == ENOENT || errno == ENODEV || errno == ENXIO) {
			fprintf(stderr, "DRM tests skipped: %s (%s)\n",
				DRM_DEVICE, strerror(errno));
			exit(EXIT_SUCCESS);
		}
		fprintf(stderr,
			"fatal error: setup_open_drm_device: open('%s') failed: %s\n",
			DRM_DEVICE, strerror(errno));
		exit(EXIT_FAILURE);
	}
	printf("opened %s\n", DRM_DEVICE);
}
END_SETUP()

FN_TEST(create_dumb_buffer)
{
	struct drm_mode_create_dumb create = {
		.width = TEST_WIDTH,
		.height = TEST_HEIGHT,
		.bpp = TEST_BPP,
		.flags = 0,
		.handle = 0,
		.pitch = 0,
		.size = 0,
	};

	TEST_SUCC(ioctl(drm_fd, DRM_IOCTL_MODE_CREATE_DUMB, &create));

	printf("CREATE_DUMB: %dx%dx%d, handle=%u, pitch=%u, size=%lu\n",
		create.width, create.height, create.bpp,
		create.handle, create.pitch, create.size);

	TEST_RES(create.handle != 0, _ret == true);
	TEST_RES(create.pitch != 0, _ret == true);
	TEST_RES(create.size != 0, _ret == true);

	dumb_handle = create.handle;
	mapped_size = create.size;
}
END_TEST()

FN_TEST(map_dumb_buffer)
{
	struct drm_mode_map_dumb map = {
		.handle = dumb_handle,
		.pad = 0,
		.offset = 0,
	};

	TEST_SUCC(ioctl(drm_fd, DRM_IOCTL_MODE_MAP_DUMB, &map));

	printf("MAP_DUMB: handle=%u, offset=%lu\n", map.handle, map.offset);

	TEST_RES(map.offset != 0, _ret == true);

	dumb_offset = map.offset;
}
END_TEST()

FN_TEST(mmap_dumb_buffer)
{
	mapped_addr = mmap(NULL, mapped_size, PROT_READ | PROT_WRITE,
		MAP_SHARED, drm_fd, dumb_offset);

	TEST_RES(mapped_addr != MAP_FAILED, _ret == true);

	printf("mmap succeeded at %p, size=%zu\n", mapped_addr, mapped_size);
}
END_TEST()

FN_TEST(write_and_read_back)
{
	/* Write a simple pattern to the buffer */
	for (size_t i = 0; i < mapped_size && i < 256; i++) {
		mapped_addr[i] = (uint8_t)(i & 0xff);
	}

	/* Read it back and verify */
	for (size_t i = 0; i < mapped_size && i < 256; i++) {
		if (mapped_addr[i] != (uint8_t)(i & 0xff)) {
			fprintf(stderr, "verify failed at offset %zu: expected 0x%02x, got 0x%02x\n",
				i, (uint8_t)(i & 0xff), mapped_addr[i]);
			__tests_failed++;
			return;
		}
	}
	printf("write/read verification passed\n");
	__tests_passed++;
}
END_TEST()

FN_TEST(destroy_dumb_buffer)
{
	struct drm_mode_destroy_dumb destroy = {
		.handle = dumb_handle,
	};

	TEST_SUCC(ioctl(drm_fd, DRM_IOCTL_MODE_DESTROY_DUMB, &destroy));

	printf("DESTROY_DUMB: handle=%u destroyed\n", dumb_handle);
}
END_TEST()

FN_SETUP(cleanup_mmap)
{
	if (mapped_addr != NULL && mapped_addr != MAP_FAILED) {
		CHECK(munmap(mapped_addr, mapped_size));
		mapped_addr = NULL;
	}
}
END_SETUP()

FN_SETUP(close_drm_device)
{
	CHECK(close(drm_fd));
	drm_fd = -1;
}
END_SETUP()
