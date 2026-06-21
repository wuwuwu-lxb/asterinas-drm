// SPDX-License-Identifier: MPL-2.0

#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <linux/kd.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

#if __has_include(<drm/drm.h>)
#include <drm/drm.h>
#include <drm/drm_mode.h>
#elif __has_include(<libdrm/drm.h>)
#include <libdrm/drm.h>
#include <libdrm/drm_mode.h>
#else
#error "DRM UAPI headers are required"
#endif

#ifndef DRM_MODE_CONNECTED
#define DRM_MODE_CONNECTED 1
#endif

#define DRM_DEVICE "/dev/dri/card0"
#define LOG_PROBE_COUNT 12
#define DISPLAY_HOLD_SECONDS 15
#define LOG_PROBE_MAGIC 0x565444524d50ULL

struct kms_target {
	uint32_t connector_id;
	uint32_t encoder_id;
	uint32_t crtc_id;
	uint32_t crtc_index;
	struct drm_mode_modeinfo mode;
};

static void fatal_errno(const char *context, const char *operation)
{
	fprintf(stderr, "fatal error: %s: %s failed: %s\n", context,
		operation, strerror(errno));
	exit(EXIT_FAILURE);
}

static void fatal_message(const char *context, const char *message)
{
	fprintf(stderr, "fatal error: %s: %s\n", context, message);
	exit(EXIT_FAILURE);
}

static int try_ioctl(int fd, unsigned long request, void *arg)
{
	int ret;

	do {
		ret = ioctl(fd, request, arg);
	} while (ret < 0 && errno == EINTR);

	return ret;
}

static void checked_ioctl(int fd, unsigned long request, void *arg,
			  const char *context, const char *name)
{
	if (try_ioctl(fd, request, arg) < 0) {
		fatal_errno(context, name);
	}
}

static const char *kd_mode_name(int mode)
{
	switch (mode) {
	case KD_TEXT:
		return "KD_TEXT";
	case KD_GRAPHICS:
		return "KD_GRAPHICS";
	default:
		return "unknown";
	}
}

static void print_console_mode(const char *stage)
{
	int tty = open("/dev/tty0", O_RDONLY | O_CLOEXEC);
	if (tty < 0) {
		printf("[%s] /dev/tty0 unavailable: %s\n", stage,
		       strerror(errno));
		return;
	}

	int mode = -1;
	if (ioctl(tty, KDGETMODE, &mode) == 0) {
		printf("[%s] console mode: %s (%d)\n", stage, kd_mode_name(mode),
		       mode);
	} else {
		printf("[%s] KDGETMODE failed: %s\n", stage, strerror(errno));
	}

	close(tty);
}

static void get_resources(int fd, struct drm_mode_card_res *res,
			  uint32_t **connectors, uint32_t **encoders,
			  uint32_t **crtcs)
{
	memset(res, 0, sizeof(*res));
	checked_ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, res, "get_resources",
		      "DRM_IOCTL_MODE_GETRESOURCES(count)");

	*connectors = calloc(res->count_connectors, sizeof(uint32_t));
	*encoders = calloc(res->count_encoders, sizeof(uint32_t));
	*crtcs = calloc(res->count_crtcs, sizeof(uint32_t));
	if ((res->count_connectors && *connectors == NULL) ||
	    (res->count_encoders && *encoders == NULL) ||
	    (res->count_crtcs && *crtcs == NULL)) {
		fatal_errno("get_resources", "calloc");
	}

	res->connector_id_ptr = (uintptr_t)*connectors;
	res->encoder_id_ptr = (uintptr_t)*encoders;
	res->crtc_id_ptr = (uintptr_t)*crtcs;

	checked_ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, res, "get_resources",
		      "DRM_IOCTL_MODE_GETRESOURCES(ids)");

	printf("drm resources: connectors=%u encoders=%u crtcs=%u fbs=%u\n",
	       res->count_connectors, res->count_encoders, res->count_crtcs,
	       res->count_fbs);
}

static bool get_connector(int fd, uint32_t connector_id,
			  struct drm_mode_get_connector *conn,
			  struct drm_mode_modeinfo **modes,
			  uint32_t **encoders)
{
	memset(conn, 0, sizeof(*conn));
	conn->connector_id = connector_id;

	if (try_ioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, conn) < 0) {
		fprintf(stderr,
			"warning: get_connector: DRM_IOCTL_MODE_GETCONNECTOR(count) failed for connector %u: %s\n",
			connector_id, strerror(errno));
		return false;
	}

	*modes = calloc(conn->count_modes, sizeof(**modes));
	*encoders = calloc(conn->count_encoders, sizeof(**encoders));
	if ((conn->count_modes && *modes == NULL) ||
	    (conn->count_encoders && *encoders == NULL)) {
		fatal_errno("get_connector", "calloc");
	}

	conn->modes_ptr = (uintptr_t)*modes;
	conn->encoders_ptr = (uintptr_t)*encoders;

	if (try_ioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, conn) < 0) {
		fprintf(stderr,
			"warning: get_connector: DRM_IOCTL_MODE_GETCONNECTOR(data) failed for connector %u: %s\n",
			connector_id, strerror(errno));
		free(*modes);
		free(*encoders);
		return false;
	}

	return true;
}

static bool crtc_index_for_id(const struct drm_mode_card_res *res,
			      const uint32_t *crtcs, uint32_t crtc_id,
			      uint32_t *index)
{
	for (uint32_t i = 0; i < res->count_crtcs; i++) {
		if (crtcs[i] == crtc_id) {
			*index = i;
			return true;
		}
	}

	return false;
}

static bool choose_crtc_from_encoder(int fd, const struct drm_mode_card_res *res,
				     const uint32_t *crtcs, uint32_t encoder_id,
				     uint32_t *crtc_id, uint32_t *crtc_index)
{
	struct drm_mode_get_encoder enc = {
		.encoder_id = encoder_id,
	};

	if (try_ioctl(fd, DRM_IOCTL_MODE_GETENCODER, &enc) < 0) {
		fprintf(stderr,
			"warning: choose_crtc_from_encoder: DRM_IOCTL_MODE_GETENCODER failed for encoder %u: %s\n",
			encoder_id, strerror(errno));
		return false;
	}

	if (enc.crtc_id != 0 &&
	    crtc_index_for_id(res, crtcs, enc.crtc_id, crtc_index)) {
		*crtc_id = enc.crtc_id;
		return true;
	}

	for (uint32_t i = 0; i < res->count_crtcs; i++) {
		if (enc.possible_crtcs & (1u << i)) {
			*crtc_id = crtcs[i];
			*crtc_index = i;
			return true;
		}
	}

	return false;
}

static struct kms_target find_target(int fd)
{
	struct drm_mode_card_res res;
	uint32_t *connector_ids = NULL;
	uint32_t *encoder_ids = NULL;
	uint32_t *crtc_ids = NULL;
	struct kms_target target;
	bool found = false;

	memset(&target, 0, sizeof(target));
	get_resources(fd, &res, &connector_ids, &encoder_ids, &crtc_ids);

	for (uint32_t i = 0; i < res.count_connectors && !found; i++) {
		struct drm_mode_get_connector conn;
		struct drm_mode_modeinfo *modes = NULL;
		uint32_t *conn_encoders = NULL;

		if (!get_connector(fd, connector_ids[i], &conn, &modes,
				   &conn_encoders)) {
			continue;
		}

		printf("connector %u: connection=%u modes=%u encoders=%u encoder=%u\n",
		       conn.connector_id, conn.connection, conn.count_modes,
		       conn.count_encoders, conn.encoder_id);

		if (conn.connection == DRM_MODE_CONNECTED && conn.count_modes > 0) {
			uint32_t crtc_id = 0;
			uint32_t crtc_index = 0;
			uint32_t chosen_encoder = conn.encoder_id;

			if (chosen_encoder != 0 &&
			    choose_crtc_from_encoder(fd, &res, crtc_ids,
						     chosen_encoder, &crtc_id,
						     &crtc_index)) {
				found = true;
			}

			for (uint32_t j = 0; j < conn.count_encoders && !found;
			     j++) {
				chosen_encoder = conn_encoders[j];
				if (choose_crtc_from_encoder(fd, &res, crtc_ids,
							     chosen_encoder,
							     &crtc_id,
							     &crtc_index)) {
					found = true;
				}
			}

			if (found) {
				target.connector_id = conn.connector_id;
				target.encoder_id = chosen_encoder;
				target.crtc_id = crtc_id;
				target.crtc_index = crtc_index;
				target.mode = modes[0];
			}
		}

		free(modes);
		free(conn_encoders);
	}

	free(connector_ids);
	free(encoder_ids);
	free(crtc_ids);

	if (!found) {
		fatal_message("find_target", "no connected KMS target found");
	}

	printf("chosen target: connector=%u encoder=%u crtc=%u mode=%ux%u %s\n",
	       target.connector_id, target.encoder_id, target.crtc_id,
	       target.mode.hdisplay, target.mode.vdisplay, target.mode.name);

	return target;
}

static void draw_pattern(uint32_t *pixels, uint32_t width, uint32_t height,
			 uint32_t pitch)
{
	uint32_t stride = pitch / sizeof(uint32_t);
	uint32_t color = 0x0000ff00;

	for (uint32_t y = 0; y < height; y++) {
		for (uint32_t x = 0; x < width; x++) {
			pixels[y * stride + x] = color;
		}
	}
}

static void sleep_msec(long msec)
{
	struct timespec ts = {
		.tv_sec = msec / 1000,
		.tv_nsec = (msec % 1000) * 1000 * 1000,
	};

	while (nanosleep(&ts, &ts) < 0 && errno == EINTR) {
	}
}

static void trigger_kernel_logs(unsigned int count)
{
	printf("triggering %u syscall-334 log probes\n", count);
	fflush(stdout);

#if defined(__x86_64__)
	for (unsigned int i = 0; i < count; i++) {
		errno = 0;
		long ret = syscall(334, LOG_PROBE_MAGIC, i, 0, 0);
		printf("probe %u/%u: syscall(334) returned %ld errno=%d (%s)\n",
		       i + 1, count, ret, errno, strerror(errno));
		fflush(stdout);
		sleep_msec(250);
	}
#else
	printf("syscall-334 probe skipped on this architecture\n");
#endif
}

int main(int argc, char **argv)
{
	const char *card = argc > 1 ? argv[1] : DRM_DEVICE;

	printf("DRM/console conflict test\n");
	printf("device: %s\n", card);
	print_console_mode("before-open");

	int fd = open(card, O_RDWR | O_CLOEXEC);
	if (fd < 0) {
		fatal_errno("open_drm_card", "open");
	}

	if (try_ioctl(fd, DRM_IOCTL_SET_MASTER, NULL) < 0) {
		printf("DRM_IOCTL_SET_MASTER failed: %s; continuing if the file is already master\n",
		       strerror(errno));
	}

	struct kms_target target = find_target(fd);

	struct drm_mode_create_dumb create = {
		.width = target.mode.hdisplay,
		.height = target.mode.vdisplay,
		.bpp = 32,
	};
	checked_ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &create,
		      "create_dumb_buffer", "DRM_IOCTL_MODE_CREATE_DUMB");

	struct drm_mode_fb_cmd fb = {
		.width = create.width,
		.height = create.height,
		.pitch = create.pitch,
		.bpp = 32,
		.depth = 24,
		.handle = create.handle,
	};
	checked_ioctl(fd, DRM_IOCTL_MODE_ADDFB, &fb, "add_framebuffer",
		      "DRM_IOCTL_MODE_ADDFB");

	struct drm_mode_map_dumb map = {
		.handle = create.handle,
	};
	checked_ioctl(fd, DRM_IOCTL_MODE_MAP_DUMB, &map, "map_dumb_buffer",
		      "DRM_IOCTL_MODE_MAP_DUMB");

	void *addr = mmap(NULL, create.size, PROT_READ | PROT_WRITE, MAP_SHARED,
			  fd, map.offset);
	if (addr == MAP_FAILED) {
		fatal_errno("map_dumb_buffer", "mmap");
	}

	draw_pattern(addr, create.width, create.height, create.pitch);

	uint32_t connector_id = target.connector_id;
	struct drm_mode_crtc set = {
		.set_connectors_ptr = (uintptr_t)&connector_id,
		.count_connectors = 1,
		.crtc_id = target.crtc_id,
		.fb_id = fb.fb_id,
		.x = 0,
		.y = 0,
		.mode_valid = 1,
		.mode = target.mode,
	};

	print_console_mode("before-setcrtc");
	checked_ioctl(fd, DRM_IOCTL_MODE_SETCRTC, &set, "set_crtc",
		      "DRM_IOCTL_MODE_SETCRTC");
	print_console_mode("after-setcrtc");

	printf("A colored DRM test pattern should now be visible.\n");
	printf("If VT console is correctly in graphics mode, following kernel logs must not overwrite it.\n");
	trigger_kernel_logs(LOG_PROBE_COUNT);
	print_console_mode("after-log-probe");

	printf("holding DRM master for %u seconds; inspect or record the QEMU/VNC display now\n",
	       DISPLAY_HOLD_SECONDS);
	sleep(DISPLAY_HOLD_SECONDS);

	munmap(addr, create.size);
	if (try_ioctl(fd, DRM_IOCTL_MODE_RMFB, &fb.fb_id) < 0) {
		fprintf(stderr,
			"warning: cleanup: DRM_IOCTL_MODE_RMFB failed: %s\n",
			strerror(errno));
	}

	struct drm_mode_destroy_dumb destroy = {
		.handle = create.handle,
	};
	if (try_ioctl(fd, DRM_IOCTL_MODE_DESTROY_DUMB, &destroy) < 0) {
		fprintf(stderr,
			"warning: cleanup: DRM_IOCTL_MODE_DESTROY_DUMB failed: %s\n",
			strerror(errno));
	}
	if (try_ioctl(fd, DRM_IOCTL_DROP_MASTER, NULL) < 0) {
		fprintf(stderr,
			"warning: cleanup: DRM_IOCTL_DROP_MASTER failed: %s\n",
			strerror(errno));
	}
	close(fd);

	print_console_mode("after-close");
	printf("done\n");
	return 0;
}
