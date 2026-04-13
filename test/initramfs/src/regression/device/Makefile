# SPDX-License-Identifier: MPL-2.0

SUBDIRS := pty

# drm_shim.c is a library module for modeset-double-buffered.c
# Filter it out from the automatic %.c -> % rule (which expects main())
C_OBJS_FILTER += drm_shim

include ../common/Makefile

# Compile drm_shim.c as a static library first
DRM_SHIM_LIB := $(OBJ_OUTPUT_DIR)/libdrm_shim.a
DRM_SHIM_OBJ := $(OBJ_OUTPUT_DIR)/drm_shim.o

$(DRM_SHIM_OBJ): drm_shim.c | $(OBJ_OUTPUT_DIR)
	$(CC) $(C_FLAGS) -c drm_shim.c -o $@
	echo "CC <= $@"

$(DRM_SHIM_LIB): $(DRM_SHIM_OBJ)
	$(AR) rcs $@ $<
	echo "AR <= $@"

# modeset-double-buffered.c is a standalone program (has its own main).
# Link with libdrm_shim.a.
.PHONY: all
all: $(C_OBJS) $(ASM_OBJS) $(SUBDIRS) $(SCRIPTS) $(OBJ_OUTPUT_DIR)/modeset-double-buffered

$(OBJ_OUTPUT_DIR)/modeset-double-buffered: modeset-double-buffered.c $(DRM_SHIM_LIB)
	$(CC) $(C_FLAGS) modeset-double-buffered.c $(DRM_SHIM_LIB) -o $@
	echo "CC <= $@"
