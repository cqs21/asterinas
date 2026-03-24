# SPDX-License-Identifier: MPL-2.0

LTP_PREBUILT_DIR ?= /opt/ltp
MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
CUR_DIR := $(patsubst %/,%,$(dir $(MKFILE_PATH)))
INITRAMFS ?= $(CUR_DIR)/../../../build/initramfs
TARGET_DIR := $(INITRAMFS)/opt/ltp
RUN_BASH := $(CUR_DIR)/run_ltp_test.sh
ALL_TESTS := $(CUR_DIR)/testcases/all.txt
EXT2_BLOCKLIST := $(CUR_DIR)/testcases/blocked/ext2.txt
EXFAT_BLOCKLIST := $(CUR_DIR)/testcases/blocked/exfat.txt

.PHONY: all
all: $(TARGET_DIR)

$(TARGET_DIR): $(RUN_BASH) $(ALL_TESTS) $(EXT2_BLOCKLIST) $(EXFAT_BLOCKLIST)
	@rm -rf $@ && mkdir -p $@
	@# Prepare tests dir for test binaries
	@mkdir -p $@/testcases/bin
	@mkdir -p $@/runtest
	@awk '!/^#/ && NF' $(ALL_TESTS) > $@/all.txt
	@if [ "$(SYSCALL_TEST_WORKDIR)" = "/ext2" ]; then \
		grep -vxF -f $(EXT2_BLOCKLIST) $@/all.txt > $@/fs_filtered.txt; \
	elif [ "$(SYSCALL_TEST_WORKDIR)" = "/exfat" ]; then \
		grep -vxF -f $(EXFAT_BLOCKLIST) $@/all.txt > $@/fs_filtered.txt; \
	else \
		cp -f $@/all.txt $@/fs_filtered.txt; \
	fi;
	@cp -f $@/fs_filtered.txt $@/extra_filtered.txt
	@if [ -n "$(EXTRA_BLOCKLISTS_DIRS)" ] && [ "$(EXTRA_BLOCKLISTS_DIRS)" != "\"\"" ] && [ "$(EXTRA_BLOCKLISTS_DIRS)" != "''" ]; then \
		for extra_dir in $(EXTRA_BLOCKLISTS_DIRS); do \
			if [ -z "$$extra_dir" ] || [ "$$extra_dir" = "\"\"" ] || [ "$$extra_dir" = "''" ]; then \
				continue; \
			fi; \
			case "$$extra_dir" in \
				/*) extra_dir_path="$$extra_dir" ;; \
				*) extra_dir_path="$(CUR_DIR)/$$extra_dir" ;; \
			esac; \
			if [ ! -d "$$extra_dir_path" ]; then \
				echo "Error: EXTRA_BLOCKLISTS_DIRS entry is not a directory: $$extra_dir" >&2; \
				exit 1; \
			fi; \
			for blocklist_file in "$$extra_dir_path/all.txt"; do \
				if [ -f "$$blocklist_file" ]; then \
					awk '!/^#/ && NF { print $$1 }' "$$blocklist_file" > $@/extra_blocklist.txt; \
					if [ -s $@/extra_blocklist.txt ]; then \
						grep -vxF -f $@/extra_blocklist.txt $@/extra_filtered.txt > $@/extra_filtered.next || true; \
						mv $@/extra_filtered.next $@/extra_filtered.txt; \
					fi; \
				fi; \
			done; \
			if [ "$(SYSCALL_TEST_WORKDIR)" = "/ext2" ] && [ -f "$$extra_dir_path/ext2.txt" ]; then \
				awk '!/^#/ && NF { print $$1 }' "$$extra_dir_path/ext2.txt" > $@/extra_blocklist.txt; \
				if [ -s $@/extra_blocklist.txt ]; then \
					grep -vxF -f $@/extra_blocklist.txt $@/extra_filtered.txt > $@/extra_filtered.next || true; \
					mv $@/extra_filtered.next $@/extra_filtered.txt; \
				fi; \
			elif [ "$(SYSCALL_TEST_WORKDIR)" = "/exfat" ] && [ -f "$$extra_dir_path/exfat.txt" ]; then \
				awk '!/^#/ && NF { print $$1 }' "$$extra_dir_path/exfat.txt" > $@/extra_blocklist.txt; \
				if [ -s $@/extra_blocklist.txt ]; then \
					grep -vxF -f $@/extra_blocklist.txt $@/extra_filtered.txt > $@/extra_filtered.next || true; \
					mv $@/extra_filtered.next $@/extra_filtered.txt; \
				fi; \
			fi; \
		done; \
	fi;
	@if [ -n "$(LTP_CASES)" ] || [ -n "$(LTP_CASE_FILE)" ]; then \
		: > $@/selected.txt; \
		if [ -n "$(LTP_CASES)" ]; then \
			printf '%s\n' "$(LTP_CASES)" | tr ',' '\n' | sed 's/^[[:space:]]*//;s/[[:space:]]*$$//' | awk 'NF' >> $@/selected.txt; \
		fi; \
		if [ -n "$(LTP_CASE_FILE)" ]; then \
			if [ ! -f "$(LTP_CASE_FILE)" ]; then \
				echo "Error: LTP_CASE_FILE does not exist: $(LTP_CASE_FILE)" >&2; \
				exit 1; \
			fi; \
			awk '!/^#/ && NF { print $$1 }' "$(LTP_CASE_FILE)" >> $@/selected.txt; \
		fi; \
		sort -u -o $@/selected.txt $@/selected.txt; \
		if [ ! -s $@/selected.txt ]; then \
			echo "Error: no testcases selected from LTP_CASES/LTP_CASE_FILE." >&2; \
			exit 1; \
		fi; \
		grep -xF -f $@/selected.txt $@/extra_filtered.txt > $@/filtered.txt || true; \
		if [ ! -s $@/filtered.txt ]; then \
			echo "Error: selected LTP cases are not enabled after blocklist filtering." >&2; \
			exit 1; \
		fi; \
	else \
		cp -f $@/extra_filtered.txt $@/filtered.txt; \
	fi;
	@# Process syscall testcases and copy binaries
	@while read -r syscall binary params; do \
		if grep -q "^$$syscall$$" $@/filtered.txt; then \
			if [ -f $(LTP_PREBUILT_DIR)/testcases/bin/$$binary ]; then \
				cp -f $(LTP_PREBUILT_DIR)/testcases/bin/$$binary $@/testcases/bin; \
				echo "$$syscall $$binary $$params" >> $@/runtest/syscalls; \
			else \
				echo "Warning: $$binary not found (skipping)"; \
			fi; \
		fi; \
	done < $(LTP_PREBUILT_DIR)/runtest/syscalls
	@# Remove intermediate files
	@rm -f $@/all.txt $@/fs_filtered.txt $@/extra_filtered.txt $@/extra_filtered.next \
		$@/filtered.txt $@/selected.txt $@/extra_blocklist.txt
	@# Copy bash scripts
	@cp -r $(LTP_PREBUILT_DIR)/bin $@
	@cp -f $(LTP_PREBUILT_DIR)/runltp $@
	@cp -f $(LTP_PREBUILT_DIR)/Version $@
	@cp -f $(LTP_PREBUILT_DIR)/ver_linux $@
	@cp -f $(LTP_PREBUILT_DIR)/IDcheck.sh $@
	@cp -f $(RUN_BASH) $@

.PHONY: clean
clean:
	@rm -rf $(TARGET_DIR)
