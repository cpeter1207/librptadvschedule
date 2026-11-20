/** @file
 * @brief Exercise the checked recurrence table through its installed C contract.
 */
#include "rptadvschedule/recurrence.h"

#include <assert.h>
#include <stddef.h>

#define TEXT(s) ((const uint8_t *)(s)), ((uint32_t)(sizeof(s) - 1))

int main(void) {
  const struct rptadv_recurrence_descriptor *api = rptadv_recurrence_descriptor();
  assert(rptadv_recurrence_descriptor_valid(api));
  assert(api == rptadv_recurrence_descriptor());
  assert(!rptadv_recurrence_descriptor_valid(NULL));
  struct rptadv_recurrence_descriptor bad = *api;
  bad.struct_size = sizeof(uint32_t);
  assert(!rptadv_recurrence_descriptor_valid(&bad));
  bad = *api;
  bad.abi_version++;
  assert(!rptadv_recurrence_descriptor_valid(&bad));
  bad = *api;
  bad.capability_name = "rptadv.schedule";
  assert(!rptadv_recurrence_descriptor_valid(&bad));
  bad = *api;
  bad.previous_before = NULL;
  assert(!rptadv_recurrence_descriptor_valid(&bad));
  assert(api->min_second == -INT64_C(62135510400));
  assert(api->end_second == INT64_C(253402128000));

  struct rptadv_recurrence *original, *copy, *same;
  assert(api->create(TEXT("every day at 0:00 in UTC"), NULL, 0, &original) == 0);
  assert(original && strcmp(api->effective_zone(original), "UTC") == 0);
  assert(api->clone_handle(original, &copy) == 0 && copy);
  assert(api->create(TEXT("EVERY DAY AT 00:00"), TEXT("UTC"), &same) == 0);
  uint32_t equal;
  assert(api->equal(same, original, &equal) == 0 && equal == 1);
  api->destroy(original);
  assert(api->equal(copy, same, &equal) == 0 && equal == 1);
  api->destroy(same);
  int64_t out;
  assert(api->match_minute(copy, 41, &out) == RPTADV_RECURRENCE_PRESENT && out == 0);
  assert(api->next_after(copy, 0, &out) == RPTADV_RECURRENCE_PRESENT && out == 86400);
  assert(api->previous_before(copy, 0, &out) == RPTADV_RECURRENCE_PRESENT && out == -86400);
  assert(api->match_minute(copy, 60, &out) == RPTADV_RECURRENCE_OK && out == 0);
  assert(api->next_after(copy, api->end_second, &out) == RPTADV_RECURRENCE_RANGE && out == 0);
  assert(api->previous_before(NULL, 0, &out) == RPTADV_RECURRENCE_ARGUMENT && out == 0);
  assert(api->match_minute(copy, 0, NULL) == RPTADV_RECURRENCE_ARGUMENT);
  assert(api->effective_zone(NULL) == NULL);
  assert(api->equal(NULL, copy, &equal) == RPTADV_RECURRENCE_ARGUMENT && equal == 0);
  assert(api->equal(copy, NULL, &equal) == RPTADV_RECURRENCE_ARGUMENT && equal == 0);
  assert(api->equal(copy, copy, NULL) == RPTADV_RECURRENCE_ARGUMENT);
  assert(api->clone_handle(copy, NULL) == RPTADV_RECURRENCE_ARGUMENT);
  original = copy;
  assert(api->clone_handle(NULL, &original) == RPTADV_RECURRENCE_ARGUMENT && !original);
  assert(api->create(TEXT("every day at 00:00"), TEXT("UTC"), NULL) == RPTADV_RECURRENCE_ARGUMENT);
  original = copy;
  assert(api->create(NULL, 4, TEXT("UTC"), &original) == RPTADV_RECURRENCE_ARGUMENT && !original);
  assert(api->create(TEXT(""), TEXT("UTC"), &original) == RPTADV_RECURRENCE_TEXT && !original);
  assert(api->create(TEXT("every day at 00:00\0zone"), TEXT("UTC"), &original) == RPTADV_RECURRENCE_TEXT);
  assert(api->create(TEXT("\xff"), TEXT("UTC"), &original) == RPTADV_RECURRENCE_TEXT);
  assert(api->create((const uint8_t *)"x", 16385, TEXT("UTC"), &original) == RPTADV_RECURRENCE_TEXT);
  assert(api->create(TEXT("every day at 00:00"), (const uint8_t *)"x", 256, &original) == RPTADV_RECURRENCE_TEXT);
  assert(api->create(TEXT("every day at 00:00"), NULL, 0, &original) == RPTADV_RECURRENCE_ZONE);
  assert(api->create(TEXT("every day at 00:00"), NULL, 3, &original) == RPTADV_RECURRENCE_ARGUMENT);
  assert(api->create(TEXT("every day at 00:00"), TEXT("UTC\0x"), &original) == RPTADV_RECURRENCE_TEXT);
  assert(api->create(TEXT("bad syntax"), TEXT("UTC"), &original) == RPTADV_RECURRENCE_PARSE);
  char source[4097];
  memset(source, ' ', sizeof(source));
  memcpy(source, "every day at 00:00", sizeof("every day at 00:00") - 1);
  assert(api->create((const uint8_t *)source, 4097, TEXT("UTC"), &original) == RPTADV_RECURRENCE_TEXT);
  assert(api->create((const uint8_t *)source, 4096, TEXT("UTC"), &original) == 0);
  api->destroy(original);
  struct rptadv_recurrence *failed, *left, *right;
  assert(api->create(TEXT("broken"), TEXT("UTC"), &failed) == RPTADV_RECURRENCE_PARSE && !failed);
  assert(api->create(TEXT("every day at 17:00, 09:00, 17:00"), TEXT("UTC"), &left) == 0);
  assert(api->create(TEXT("every day at 09:00, 17:00"), TEXT("UTC"), &right) == 0);
  assert(api->equal(left, right, &equal) == 0 && equal == 1);
  assert(api->equal(left, copy, &equal) == 0 && equal == 0);
  api->destroy(right);
  api->destroy(left);
  api->destroy(copy);
  api->destroy(NULL);
  return 0;
}
