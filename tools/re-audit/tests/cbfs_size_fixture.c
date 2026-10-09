/* SPDX-License-Identifier: GPL-2.0-only */
/* Offline backing for actual cbfs.c definitions inserted at SOURCE_FUNCTIONS.
 * Decoder outcomes, storage, hashes and allocation are controlled stubs.
 * No real decoder, CBFS image, firmware, TPM, PCI or device is accessed.
 */
#include <assert.h>
#include <endian.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum cb_err { CB_SUCCESS = 0, CB_ERR = 1 };
enum cbfs_type { CBFS_TYPE_QUERY = 0, CBFS_TYPE_OPTIONROM = 0x30, CBFS_TYPE_RAW = 0x50 };
enum { CBFS_COMPRESS_NONE = 0, CBFS_COMPRESS_LZMA = 1,
	CBFS_COMPRESS_LZ4 = 2, CBFS_COMPRESS_ZSTD = 3 };
enum { CBFS_FILE_ATTR_TAG_COMPRESSION = 0x42435a4c };
enum { TS_ULZ4F_START, TS_ULZ4F_END, TS_ULZMA_START, TS_ULZMA_END,
	TS_UZSTDF_START, TS_UZSTDF_END };

struct region_device { const void *data; size_t size; };
struct cbfs_file_attr_compression {
	uint32_t tag, len, compression, decompressed_size;
};
union cbfs_mdata {
	struct { uint32_t type; char filename[32]; } h;
};
typedef void *(*cbfs_allocator_t)(void *, size_t, const union cbfs_mdata *);
struct mem_pool { size_t size; };
static struct mem_pool cbfs_cache = { .size = 4096 };

#define CONFIG_CBFS_VERIFICATION 1
#define CONFIG_CBFS_ALLOW_UNVERIFIED_DECOMPRESSION 0
#define CONFIG(x) CONFIG_##x
/* The actual function expressions remain evaluated, without console output. */
#define DEBUG(...) fixture_log(__VA_ARGS__)
#define ERROR(...) fixture_log(__VA_ARGS__)
static void fixture_log(const char *format, ...) { (void)format; }

static unsigned algorithm = CBFS_COMPRESS_LZ4;
static size_t raw_size = 32, capacity = 1024, produced_size = 512;
static uint8_t *raw_mapping, *allocated;
static bool has_attribute = true, fail_map, fail_hash, fail_allocation, fail_read;
static bool fail_lookup, preload;
static unsigned lookup_calls, maps, unmaps, hash_calls, decode_calls, read_calls;
static unsigned allocation_calls, custom_calls, preload_calls;
static size_t allocation_size, decoder_capacity, hash_size;
static struct cbfs_file_attr_compression attribute;

static size_t region_device_sz(const struct region_device *rdev) { return rdev->size; }
static const void *cbfs_find_attr(const union cbfs_mdata *mdata, uint32_t tag, size_t size)
{
	(void)mdata;
	assert(tag == CBFS_FILE_ATTR_TAG_COMPRESSION && size == sizeof(attribute));
	return has_attribute ? &attribute : NULL;
}
static void *rdev_mmap_full(const struct region_device *rdev)
{
	maps++;
	assert(rdev->data == raw_mapping);
	return fail_map ? NULL : raw_mapping;
}
static void rdev_munmap(const struct region_device *rdev, void *mapping)
{
	assert(rdev->data == raw_mapping && mapping == raw_mapping);
	unmaps++;
}
static void cbfs_unmap(void *mapping) { assert(mapping == raw_mapping); unmaps++; }
static bool cbfs_file_hash_mismatch(const void *buffer, size_t size,
				  const union cbfs_mdata *mdata, bool skip_verification)
{
	(void)mdata; (void)skip_verification;
	assert(buffer == raw_mapping || buffer == allocated);
	hash_calls++;
	hash_size = size; /* Record the actual source's requested hash extent. */
	return fail_hash;
}
static size_t rdev_readat(const struct region_device *rdev, void *buffer,
			 size_t offset, size_t size)
{
	read_calls++;
	assert(offset == 0 && size == rdev->size && buffer == allocated);
	if (fail_read)
		return 0;
	assert(size <= allocation_size);
	memcpy(buffer, rdev->data, size);
	return size;
}
static void *allocate(size_t size)
{
	allocation_calls++;
	allocation_size = size;
	if (fail_allocation)
		return NULL;
	allocated = malloc(size ? size : 1);
	assert(allocated);
	memset(allocated, 0xcc, size ? size : 1);
	return allocated;
}
static void *mem_pool_alloc(struct mem_pool *pool, size_t size)
{
	assert(pool == &cbfs_cache);
	return allocate(size);
}
static void *custom_allocator(void *arg, size_t size, const union cbfs_mdata *mdata)
{
	assert(arg == &attribute && mdata);
	custom_calls++;
	return allocate(size);
}
static bool cbfs_lz4_enabled(void) { return true; }
static bool cbfs_lzma_enabled(void) { return true; }
static bool cbfs_zstd_enabled(void) { return true; }
static void timestamp_add_now(int event) { (void)event; }
static size_t decoder(const void *src, size_t src_size, void *dst, size_t dst_size)
{
	assert(src == raw_mapping && src_size == raw_size && dst == allocated);
	decode_calls++;
	decoder_capacity = dst_size;
	/* Impossible/error return values must not make the stub write outside
	 * backing: the actual allocator's result interpretation is under test. */
	if (produced_size <= dst_size)
		memset(dst, 0x5a, produced_size);
	return produced_size;
}
static size_t ulz4fn(const void *src, size_t src_size, void *dst, size_t dst_size)
{ return decoder(src, src_size, dst, dst_size); }
static size_t ulzman(const void *src, size_t src_size, void *dst, size_t dst_size)
{ return decoder(src, src_size, dst, dst_size); }
static size_t uzstdn(const void *src, size_t src_size, void *dst, size_t dst_size)
{ return decoder(src, src_size, dst, dst_size); }
static enum cb_err _cbfs_boot_lookup(const char *name, bool force_ro,
				     union cbfs_mdata *mdata, struct region_device *rdev)
{
	(void)force_ro;
	lookup_calls++;
	assert(!strcmp(name, "fixture"));
	if (fail_lookup)
		return CB_ERR;
	mdata->h.type = htobe32(CBFS_TYPE_RAW);
	strcpy(mdata->h.filename, name);
	*rdev = (struct region_device) { .data = raw_mapping, .size = raw_size };
	return CB_SUCCESS;
}
static enum cb_err get_preload_rdev(struct region_device *rdev, const char *name)
{
	assert(rdev->data == raw_mapping && !strcmp(name, "fixture"));
	preload_calls++;
	return preload ? CB_SUCCESS : CB_ERR;
}

/* SOURCE_FUNCTIONS */

int main(int argc, char **argv)
{
	assert(argc == 2);
	const char *scenario = argv[1];
	bool custom = false, force_ro = false, null_output = false;
	enum cbfs_type type = CBFS_TYPE_RAW;
	if (!strncmp(scenario, "direct", 6) || !strncmp(scenario, "none", 4) ||
	    !strncmp(scenario, "custom-none", 11) || !strcmp(scenario, "custom-empty") ||
	    !strcmp(scenario, "custom-hash-failure") || !strcmp(scenario, "raw-read-failure")) {
		algorithm = CBFS_COMPRESS_NONE;
		raw_size = 512;
		has_attribute = strstr(scenario, "none") != NULL;
	}
	if (!strcmp(scenario, "none-undersized") || !strcmp(scenario, "custom-none-undersized"))
		capacity = 128;
	if (!strcmp(scenario, "none-zero-attribute"))
		capacity = 0;
	if (!strncmp(scenario, "custom", 6) || !strcmp(scenario, "compressed-short-custom") ||
	    !strcmp(scenario, "raw-read-failure"))
		custom = true;
	if (!strcmp(scenario, "compressed-exact"))
		produced_size = capacity;
	if (!strcmp(scenario, "compressed-zero") || !strcmp(scenario, "preload-decode-failure"))
		produced_size = 0;
	if (!strcmp(scenario, "compressed-over-capacity"))
		produced_size = capacity + 1;
	if (!strcmp(scenario, "zstd-size-error")) {
		algorithm = CBFS_COMPRESS_ZSTD;
		produced_size = SIZE_MAX;
	}
	if (!strcmp(scenario, "lzma-short"))
		algorithm = CBFS_COMPRESS_LZMA;
	if (!strcmp(scenario, "direct-empty") || !strcmp(scenario, "custom-empty"))
		raw_size = 0;
	if (!strcmp(scenario, "compressed-empty-capacity"))
		capacity = produced_size = 0;
	if (!strcmp(scenario, "direct-map-failure") || !strcmp(scenario, "compressed-map-failure"))
		fail_map = true;
	if (!strcmp(scenario, "direct-hash-failure") || !strcmp(scenario, "compressed-hash-failure") ||
	    !strcmp(scenario, "custom-hash-failure"))
		fail_hash = true;
	if (!strcmp(scenario, "cache-allocation-failure") || !strcmp(scenario, "custom-allocation-failure"))
		fail_allocation = true;
	if (!strcmp(scenario, "cache-unavailable"))
		cbfs_cache.size = 0;
	if (!strcmp(scenario, "raw-read-failure"))
		fail_read = true;
	if (!strcmp(scenario, "lookup-failure"))
		fail_lookup = true;
	if (!strcmp(scenario, "type-failure"))
		type = CBFS_TYPE_OPTIONROM;
	if (!strcmp(scenario, "query-type"))
		type = CBFS_TYPE_QUERY;
	if (!strcmp(scenario, "force-ro"))
		force_ro = true;
	if (!strcmp(scenario, "null-output"))
		null_output = true;
	if (!strcmp(scenario, "preload-short") || !strcmp(scenario, "preload-decode-failure"))
		preload = true;
	attribute.compression = htobe32(algorithm);
	attribute.decompressed_size = htobe32(capacity);
	raw_mapping = malloc(raw_size ? raw_size : 1);
	assert(raw_mapping);
	memset(raw_mapping, 0xa5, raw_size ? raw_size : 1);
	size_t reported = 777;
	void *result = _cbfs_alloc("fixture", custom ? custom_allocator : NULL,
				  &attribute, null_output ? NULL : &reported, force_ro, &type);
	printf("{\"ok\":%s,\"reported\":%zu,\"raw_size\":%zu,\"produced\":%zu,"
	       "\"allocation_size\":%zu,\"decoder_capacity\":%zu,\"hash_size\":%zu,"
	       "\"lookups\":%u,\"maps\":%u,\"unmaps\":%u,\"hash_calls\":%u,"
	       "\"decode_calls\":%u,\"read_calls\":%u,\"allocation_calls\":%u,"
	       "\"custom_calls\":%u,\"preload_calls\":%u,\"type\":%d}\n",
	       result ? "true" : "false", reported, raw_size, produced_size,
	       allocation_size, decoder_capacity, hash_size, lookup_calls, maps, unmaps,
	       hash_calls, decode_calls, read_calls, allocation_calls, custom_calls,
	       preload_calls, type);
	/* Fixture-owned backing is freed after recording behavior. This does not
	 * add a free to the production allocator's failure or success paths. */
	free(allocated);
	free(raw_mapping);
	return 0;
}
