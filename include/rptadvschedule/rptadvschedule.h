/** @file
 * @brief Generic local civil scheduling descriptor ABI.
 */
#ifndef RPTADV_SCHEDULE_H
#define RPTADV_SCHEDULE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define RPTADV_SCHEDULE_ABI_VERSION 1U /**< Initial schedule descriptor ABI. */

/** @brief Validated local civil minute, with Sunday numbered zero. */
struct rptadv_schedule_civil {
  uint16_t year; /**< Gregorian year, 1 through 9999. */
  uint8_t month; /**< One-based month. */
  uint8_t day; /**< One-based day of month. */
  uint8_t weekday; /**< Sunday zero through Saturday six. */
  uint8_t hour; /**< Hour, zero through 23. */
  uint8_t minute; /**< Minute, zero through 59. */
};

/** @brief Parsed event record; kind 1 daily, 2 weekly, 3 one-time. */
struct rptadv_schedule_event {
  uint32_t kind; /**< 1 daily, 2 weekly, 3 one-time. */
  uint16_t year; /**< One-time year; zero otherwise. */
  uint8_t month; /**< One-time month; zero otherwise. */
  uint8_t day; /**< One-time day; zero otherwise. */
  uint8_t weekday; /**< Weekly Sunday-zero weekday; zero otherwise. */
  uint8_t hour; /**< Local hour. */
  uint8_t minute; /**< Local minute. */
};

/** @brief Opaque owned window parsed outside a real-time callback. */
struct rptadv_schedule_window;

/** @brief Immutable function table for the process lifetime.
 * All status-returning operations return zero on success and -1 for invalid
 * input or a missing required pointer. Boolean outputs use uint32_t: zero is
 * false and any nonzero value is true. Callers own each created or cloned
 * window and must destroy it exactly once after use stops.
 */
struct rptadv_schedule_descriptor {
  uint32_t struct_size; /**< Size of this descriptor. */
  uint32_t abi_version; /**< Must equal RPTADV_SCHEDULE_ABI_VERSION. */
  const char *capability_name; /**< Stable `rptadv.schedule` identity. */

  /** @brief Validate a civil minute and write its explicit record. */
  int32_t (*civil_new)(uint16_t year, uint8_t month, uint8_t day,
                       uint8_t weekday, uint8_t hour, uint8_t minute,
                       struct rptadv_schedule_civil *out);
  /** @brief Parse an exact daily, weekly or one-time event into a record. */
  int32_t (*event_parse)(const char *text, struct rptadv_schedule_event *out);
  /** @brief Calculate the event's local-minute identity if due.
   * @param present Receives 1 if due, otherwise 0.
   * @param occurrence Receives the identity if due, otherwise zero.
   */
  int32_t (*event_occurrence)(const struct rptadv_schedule_event *event,
                              const struct rptadv_schedule_civil *local,
                              uint32_t *present, uint64_t *occurrence);
  /** @brief Validate an inclusive start clock. Null returns false. */
  uint32_t (*start_time_valid)(const char *text);
  /** @brief Validate an exclusive end clock, including 24:00. Null returns false. */
  uint32_t (*end_time_valid)(const char *text);
  /** @brief Validate an optional weekday selector. Null returns false. */
  uint32_t (*weekday_selector_valid)(const char *text);
  /** @brief Validate an optional date selector. Null returns false. */
  uint32_t (*date_selector_valid)(const char *text);

  /** @brief Parse a same-date window and allocate its opaque handle.
   * @param days_present Zero selects None; nonzero requires days, including an
   *                     explicitly empty string when supplied.
   * @param dates_present Zero selects None; nonzero requires dates, including
   *                      an explicitly empty string when supplied.
   * @param out Receives a handle on success or null on parse failure.
   */
  int32_t (*window_create)(uint32_t days_present, const char *days,
                           uint32_t dates_present, const char *dates,
                           const char *start, const char *end,
                           struct rptadv_schedule_window **out);
  /** @brief Release an owned window; null is accepted. */
  void (*window_destroy)(struct rptadv_schedule_window *window);
  /** @brief Clone one owned window. */
  int32_t (*window_clone)(const struct rptadv_schedule_window *window,
                          struct rptadv_schedule_window **out);
  /** @brief Compare parsed window values. */
  int32_t (*window_equal)(const struct rptadv_schedule_window *left,
                          const struct rptadv_schedule_window *right,
                          uint32_t *equal);
  /** @brief Test whether the local civil minute is inside the window. */
  int32_t (*window_matches)(const struct rptadv_schedule_window *window,
                            const struct rptadv_schedule_civil *local,
                            uint32_t *matches);
  /** @brief Test time and calendar overlap of two windows. */
  int32_t (*window_overlaps)(const struct rptadv_schedule_window *left,
                             const struct rptadv_schedule_window *right,
                             uint32_t *overlaps);
  /** @brief Return elapsed milliseconds after a selected exclusive end.
   * @param second Local second, zero through 59; 60 produces present=0.
   * @param present Receives 1 if elapsed is meaningful, otherwise 0.
   * @param elapsed Receives milliseconds or zero when absent.
   */
  int32_t (*window_elapsed_after_end)(const struct rptadv_schedule_window *window,
                                      const struct rptadv_schedule_civil *local,
                                      uint8_t second, uint32_t *present,
                                      uint64_t *elapsed);
  /** @brief Return the next inclusive start as date identity and milliseconds.
   * An absent result zeros date_identity and remaining_milliseconds.
   */
  int32_t (*window_next_start)(const struct rptadv_schedule_window *window,
                               const struct rptadv_schedule_civil *local,
                               uint8_t second, uint32_t *present,
                               uint64_t *date_identity,
                               uint64_t *remaining_milliseconds);
  /** @brief Return the next exclusive end as date identity and milliseconds.
   * An absent result zeros date_identity and remaining_milliseconds.
   */
  int32_t (*window_next_end)(const struct rptadv_schedule_window *window,
                             const struct rptadv_schedule_civil *local,
                             uint8_t second, uint32_t *present,
                             uint64_t *date_identity,
                             uint64_t *remaining_milliseconds);
};

/** @brief Return the immutable descriptor; validate identity, size and ABI. */
const struct rptadv_schedule_descriptor *rptadv_schedule_descriptor(void);

#ifdef __cplusplus
}
#endif

#endif
