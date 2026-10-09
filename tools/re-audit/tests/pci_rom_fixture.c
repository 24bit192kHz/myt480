/* SPDX-License-Identifier: GPL-2.0-only */
/* Offline host harness: patched coreboot functions are inserted at SOURCE_FUNCTIONS.
 * These stubs provide allocated CBFS buffers and collect _ROM/CBMEM output.
 * No PCI, firmware, device, or ACPI host APIs are called.
 */
#include <assert.h>
#include <endian.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;

/* Identical structures from coreboot src/include/device/pci_rom.h. */
struct rom_header {
	uint16_t signature;
	uint8_t size;
	uint8_t init[3];
	uint8_t reserved[0x12];
	uint16_t data;
};
struct pci_data {
	uint32_t signature;
	uint16_t vendor;
	uint16_t device;
	uint16_t reserved_1;
	uint16_t dlen;
	uint8_t drevision;
	uint8_t class_lo;
	uint16_t class_hi;
	uint16_t ilen;
	uint16_t irevision;
	uint8_t type;
	uint8_t indicator;
	uint16_t reserved_2;
};
_Static_assert(sizeof(struct rom_header) == 26, "Actual ROM header ABI");
_Static_assert(sizeof(struct pci_data) == 24, "Actual PCIR base ABI");

struct device {
	uint16_t vendor, device;
	uint32_t class;
	bool enabled;
	struct rom_header *pci_vga_option_rom;
};
#define PCI_ROM_HDR 0xAA55
#define PCI_DATA_HDR 0x52494350
#define PCI_BASE_CLASS_DISPLAY 3
#define PCI_CLASS_DISPLAY_VGA 0x0300
#define PCI_ROM_ADDRESS 0x30
#define PCI_ROM_ADDRESS_ENABLE 1
#define PCI_ROM_ADDRESS_MASK (~0x7ffu)
#define CBMEM_ID_ROM0 0xf000u
#define CBMEM_ID_ROM3 (CBMEM_ID_ROM0 + 3)
#define CONFIG_ON_DEVICE_ROM_LOAD 0
#define CONFIG_CPU_QEMU_X86 0
#define CONFIG(x) CONFIG_##x
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define le16_to_cpu(x) le16toh(x)
#define le32_to_cpu(x) le32toh(x)
enum { BIOS_DEBUG, BIOS_NOTICE, BIOS_ERR, BIOS_SPEW, BIOS_WARNING };

static unsigned maps, unmaps, size_queries;
static size_t mapped_size, stored_size, copied_size, exposed_size;
static u8 *mapping, *allocation, *cbmem;
static u32 remapped_id;
static char found_name[32];
static bool fail_allocation;

static void printk(int level, const char *format, ...)
{
	/* Consume real variadic expressions so UBSAN checks field accesses used
	 * by logging as well as accesses used for decisions. */
	char text[1024];
	va_list args;
	(void)level;
	va_start(args, format);
	vsnprintf(text, sizeof(text), format, args);
	va_end(args);
}
static const char *dev_path(const struct device *dev) { (void)dev; return "fake"; }
static const char *acpi_device_path(const struct device *dev) { (void)dev; return "\\_SB.PCI0.PEG0"; }
static u32 map_oprom_vendev(u32 id) { return remapped_id ? remapped_id : id; }
static void *cbfs_map(const char *name, size_t *size)
{
	maps++;
	/* A failed lookup may leave an unrelated size; the successful fallback
	 * must replace it with the length of its own mapping. */
	if (size)
		*size = strcmp(name, found_name) ? 8192 : mapped_size;
	return strcmp(name, found_name) ? NULL : mapping;
}
static __attribute__((unused)) void cbfs_unmap(void *pointer) { assert(pointer == mapping); unmaps++; }
static __attribute__((unused)) size_t cbfs_get_size(const char *name)
{
	size_queries++;
	return strcmp(name, found_name) ? 0 : stored_size;
}
static u32 pci_read_config32(const struct device *dev, unsigned reg)
{
	(void)dev; (void)reg;
	abort(); /* Host PCI access is forbidden in this fixture. */
}
static void pci_write_config32(const struct device *dev, unsigned reg, u32 value)
{
	(void)dev; (void)reg; (void)value;
	abort();
}
static void *cbmem_add(unsigned id, size_t size)
{
	assert(id >= CBMEM_ID_ROM0 && id <= CBMEM_ID_ROM3);
	if (fail_allocation)
		return NULL;
	cbmem = malloc(size);
	assert(cbmem);
	copied_size = size;
	return cbmem;
}
static void acpigen_write_scope(const char *scope) { assert(scope); }
static void acpigen_write_rom(void *rom, size_t size)
{
	assert(rom == cbmem);
	assert(size == copied_size);
	assert(memcmp(rom, mapping, size) == 0);
	exposed_size = size;
}
static void acpigen_pop_len(void) {}

/* SOURCE_FUNCTIONS */

static void allocate_image(size_t size, bool unaligned)
{
	mapped_size = size;
	stored_size = size;
	allocation = calloc(1, (size ? size : 1) + (unaligned ? 1 : 0));
	assert(allocation);
	mapping = allocation + (unaligned ? 1 : 0);
	strcpy(found_name, "pci10de,1d10.rom");
}
static void image_at(size_t position, unsigned blocks, unsigned init_blocks, bool last)
{
	struct rom_header header = { .signature = htole16(PCI_ROM_HDR), .size = init_blocks,
		.data = htole16(0x40) };
	struct pci_data data = { .signature = htole32(PCI_DATA_HDR), .vendor = htole16(0x10de),
		.device = htole16(0x1d10), .dlen = htole16(sizeof(data)), .class_hi = htole16(0x0302),
		.ilen = htole16(blocks), .indicator = last ? 0x80 : 0 };
	assert(position + 0x40 + sizeof(data) <= mapped_size);
	memcpy(mapping + position, &header, sizeof(header));
	memcpy(mapping + position + 0x40, &data, sizeof(data));
}
static void set16(size_t offset, uint16_t value)
{
	value = htole16(value);
	assert(offset + sizeof(value) <= mapped_size);
	memcpy(mapping + offset, &value, sizeof(value));
}
static void file_image(const char *name)
{
	FILE *file = fopen(name, "rb");
	assert(file);
	assert(fseek(file, 0, SEEK_END) == 0);
	long size = ftell(file);
	assert(size >= 0);
	rewind(file);
	allocate_image((size_t)size, false);
	assert(fread(mapping, 1, mapped_size, file) == mapped_size);
	fclose(file);
}
int main(int argc, char **argv)
{
	assert(argc >= 2);
	const char *scenario = argv[1];
	struct device dev = { .vendor = 0x10de, .device = 0x1d10, .class = 0x030200, .enabled = true };
	if (!strcmp(scenario, "full-mx150")) {
		assert(argc == 3);
		file_image(argv[2]);
	} else if (!strcmp(scenario, "empty")) {
		allocate_image(0, false);
	} else if (!strcmp(scenario, "short-header")) {
		allocate_image(sizeof(struct rom_header) - 1, false);
		set16(0, PCI_ROM_HDR);
	} else {
		size_t size = 512;
		if (!strcmp(scenario, "multi-valid") || !strcmp(scenario, "compressed") ||
		    !strcmp(scenario, "multi-bad-second") || !strcmp(scenario, "pcir-crosses-image") ||
		    !strcmp(scenario, "init-over-image"))
			size = 1024;
		if (!strcmp(scenario, "multi-truncated-next"))
			size = 512 + sizeof(struct rom_header) - 1;
		allocate_image(size, !strcmp(scenario, "unaligned-map"));
		image_at(0, 1, 1, true);
		if (!strcmp(scenario, "truncated-pcir")) {
			mapped_size = 0x40 + sizeof(struct pci_data) - 1;
			/* Resize allocation as well; ASAN must see the exact mapping end. */
			allocation = realloc(allocation, mapped_size);
			assert(allocation); mapping = allocation;
		} else if (!strcmp(scenario, "bad-offset")) {
			set16(offsetof(struct rom_header, data), 0xffff);
		} else if (!strcmp(scenario, "overlapping-offset")) {
			set16(offsetof(struct rom_header, data), 4);
		} else if (!strcmp(scenario, "zero-ilen")) {
			set16(0x40 + offsetof(struct pci_data, ilen), 0);
		} else if (!strcmp(scenario, "overlength") || !strcmp(scenario, "fallback-overlength")) {
			set16(0x40 + offsetof(struct pci_data, ilen), 2);
		} else if (!strcmp(scenario, "init-over-file") || !strcmp(scenario, "init-over-image")) {
			mapping[offsetof(struct rom_header, size)] = 2;
		} else if (!strcmp(scenario, "bad-pcir-signature")) {
			mapping[0x40] = 0;
		} else if (!strcmp(scenario, "short-dlen")) {
			set16(0x40 + offsetof(struct pci_data, dlen), sizeof(struct pci_data) - 1);
		} else if (!strcmp(scenario, "long-dlen")) {
			set16(0x40 + offsetof(struct pci_data, dlen), 0xffff);
		} else if (!strcmp(scenario, "pcir-crosses-image")) {
			memmove(mapping + 500, mapping + 0x40, sizeof(struct pci_data));
			set16(offsetof(struct rom_header, data), 500);
		} else if (!strcmp(scenario, "multi-valid") || !strcmp(scenario, "multi-bad-second")) {
			image_at(0, 1, 1, false); image_at(512, 1, 1, true);
			if (!strcmp(scenario, "multi-bad-second")) mapping[512] = 0;
		} else if (!strcmp(scenario, "missing-last") || !strcmp(scenario, "multi-truncated-next")) {
			image_at(0, 1, 1, false);
		} else if (!strcmp(scenario, "compressed")) {
			image_at(0, 2, 1, true); stored_size = 32;
		} else if (!strcmp(scenario, "unaligned-pcir")) {
			memmove(mapping + 0x41, mapping + 0x40, sizeof(struct pci_data));
			set16(offsetof(struct rom_header, data), 0x41);
		}
		if (!strncmp(scenario, "fallback", 8)) remapped_id = 0x10deffff;
		if (!strcmp(scenario, "allocation-failure")) fail_allocation = true;
	}

	/* This is the actual public probe + actual SSDT implementation, not a
	 * reimplementation of its decision logic in the harness. */
	bool probe_ok = pci_rom_probe(&dev) != NULL;
	pci_rom_ssdt(&dev);
	printf("{\"probe_ok\":%s,\"copied\":%zu,\"exposed\":%zu,\"maps\":%u,"
	       "\"unmaps\":%u,\"size_queries\":%u,\"mapped_size\":%zu}\n",
	       probe_ok ? "true" : "false", copied_size, exposed_size, maps, unmaps,
	       size_queries, mapped_size);
	free(cbmem);
	free(allocation);
	return 0;
}
