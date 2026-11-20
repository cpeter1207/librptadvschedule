/** @file
 * @brief Immutable hron recurrence handles evaluated against captured Unix seconds.
 */
#ifndef RPTADV_RECURRENCE_H
#define RPTADV_RECURRENCE_H
#include <stdint.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

#define RPTADV_RECURRENCE_ABI_VERSION 1U /**< Initial recurrence ABI. */

/** @brief Static results and diagnostics. No operation echoes expression text. */
enum rptadv_recurrence_status {
  RPTADV_RECURRENCE_OK = 0, /**< Operation succeeded; query found no occurrence. */
  RPTADV_RECURRENCE_PRESENT = 1, /**< Query found an occurrence. */
  RPTADV_RECURRENCE_ARGUMENT = -1, /**< Required pointer missing. */
  RPTADV_RECURRENCE_TEXT = -2, /**< Invalid UTF-8, embedded NUL, or size limit. */
  RPTADV_RECURRENCE_ZONE = -3, /**< Required default timezone missing or unknown. */
  RPTADV_RECURRENCE_PARSE = -4, /**< Invalid hron expression or inline timezone. */
  RPTADV_RECURRENCE_RANGE = -5, /**< Instant or explicit date outside supported range. */
  RPTADV_RECURRENCE_LIMIT = -6 /**< Occurrence search reached its work limit. */
};

/** @brief Opaque owned recurrence. Destroy after concurrent readers finish. */
struct rptadv_recurrence;

/** @brief Fixed-width query: positive for present, zero for absent/exhausted.
 * A writable occurrence output receives zero on exhaustion or error. A present
 * value of zero identifies the Unix epoch. Seconds use signed POSIX time.
 */
typedef int32_t (*rptadv_recurrence_query)(const struct rptadv_recurrence *,
                                          int64_t, int64_t *);

/** @brief Recurrence ABI 1. All operations are control-plane operations. */
struct rptadv_recurrence_descriptor {
  uint32_t struct_size; /**< Table size in bytes. */
  uint32_t abi_version; /**< RPTADV_RECURRENCE_ABI_VERSION. */
  const char *capability_name; /**< Stable identity rptadv.recurrence. */
  int64_t min_second; /**< Inclusive 0001-01-02T00:00:00Z. */
  int64_t end_second; /**< Exclusive 9999-12-30T00:00:00Z. */
  /** @brief Compile an expression with an optional resolved host IANA zone.
   * Both spans exclude a terminator. Source is 1..4096 Unicode scalars and
   * at most 16384 bytes. Default zone is at most 255 bytes, or NULL/0 if
   * unavailable. An inline zone takes precedence. No implicit UTC fallback.
   * Returns zero on success and clears a writable output on failure.
   */
  int32_t (*create)(const uint8_t *source, uint32_t source_len,
                    const uint8_t *host_zone, uint32_t zone_len,
                    struct rptadv_recurrence **out);
  /** @brief Release one owned handle. NULL is accepted. */
  void (*destroy)(struct rptadv_recurrence *);
  /** @brief Return a new owned handle or clear the writable output on failure. */
  int32_t (*clone_handle)(const struct rptadv_recurrence *, struct rptadv_recurrence **);
  /** @brief Compare normalized parts, bound timezone rules, and compatibility policy. */
  int32_t (*equal)(const struct rptadv_recurrence *, const struct rptadv_recurrence *,
                   uint32_t *out);
  /** @brief Borrow the canonical NUL-terminated zone until this handle is destroyed.
   * Returns NULL for a NULL handle.
   */
  const char *(*effective_zone)(const struct rptadv_recurrence *);
  /** @brief Match the captured local minute, including a nonzero seconds field. */
  rptadv_recurrence_query match_minute;
  /** @brief Find the first occurrence strictly later than the supplied second. */
  rptadv_recurrence_query next_after;
  /** @brief Find the last occurrence strictly earlier than the supplied second.
   * At startup, match_minute includes a boundary exactly at the cursor.
   */
  rptadv_recurrence_query previous_before;
};

/** @brief Get the process-lifetime table. Validate before reading operations. */
const struct rptadv_recurrence_descriptor *rptadv_recurrence_descriptor(void);

/** @brief Validate the size, version, identity and required operations in order. */
static inline int rptadv_recurrence_descriptor_valid(
    const struct rptadv_recurrence_descriptor *table) {
  return table && table->struct_size >= sizeof(*table) &&
         table->abi_version == RPTADV_RECURRENCE_ABI_VERSION &&
         table->capability_name && !strcmp(table->capability_name, "rptadv.recurrence") &&
         table->create && table->destroy && table->clone_handle && table->equal &&
         table->effective_zone && table->match_minute && table->next_after &&
         table->previous_before;
}

#ifdef __cplusplus
}
#endif
#endif
