#include "rptadvschedule/rptadvschedule.h"

#include <assert.h>
#include <stddef.h>
#include <string.h>

int main(void) {
  const struct rptadv_schedule_descriptor *api = rptadv_schedule_descriptor();
  assert(api == rptadv_schedule_descriptor());
  assert(api->struct_size == sizeof(*api));
  assert(api->abi_version == RPTADV_SCHEDULE_ABI_VERSION);
  assert(strcmp(api->capability_name, "rptadv.schedule") == 0);
  assert(api->civil_new && api->event_parse && api->event_occurrence &&
         api->start_time_valid && api->end_time_valid &&
         api->weekday_selector_valid && api->date_selector_valid &&
         api->window_create && api->window_destroy && api->window_clone &&
         api->window_equal && api->window_matches && api->window_overlaps &&
         api->window_elapsed_after_end && api->window_next_start &&
         api->window_next_end);

  struct rptadv_schedule_civil local = {0};
  assert(api->civil_new(2028, 2, 29, 2, 9, 30, &local) == 0);
  assert(local.year == 2028 && local.weekday == 2);
  struct rptadv_schedule_event event = {0};
  assert(api->event_parse("once 2028-02-29 09:30", &event) == 0);
  uint32_t present = 0;
  uint64_t occurrence = 0;
  assert(api->event_occurrence(&event, &local, &present, &occurrence) == 0);
  assert(present == 1 && occurrence != 0);
  assert(api->start_time_valid("24:00") == 0);
  assert(api->end_time_valid("24:00") == 1);
  assert(api->weekday_selector_valid("Friday-Monday") == 1);
  assert(api->date_selector_valid("2028-02-29") == 1);

  struct rptadv_schedule_window *window = NULL;
  assert(api->window_create(0, NULL, 1, "2028-02-29", "09:00", "10:00",
                            &window) == 0);
  assert(window);
  uint32_t matches = 0;
  assert(api->window_matches(window, &local, &matches) == 0 && matches == 1);
  uint64_t date_identity = 0, remaining = 0;
  assert(api->window_next_end(window, &local, 0, &present, &date_identity,
                              &remaining) == 0);
  assert(present == 1 && date_identity != 0 && remaining == 1800000);
  struct rptadv_schedule_window *copy = NULL;
  assert(api->window_clone(window, &copy) == 0 && copy);
  uint32_t equal = 0;
  assert(api->window_equal(window, copy, &equal) == 0 && equal == 1);
  api->window_destroy(copy);
  api->window_destroy(window);
  api->window_destroy(NULL);
  return 0;
}
