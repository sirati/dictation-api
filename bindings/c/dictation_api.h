#ifndef DICTATION_API_H
#define DICTATION_API_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct DictationConnection DictationConnection;

/**
 * Borrowed metadata, copied during client registration. Null icon means no icon.
 */
typedef struct DictationApplication {
  const char *id;
  const char *name;
  const char *icon;
  uint32_t process_id;
} DictationApplication;

/**
 * Connect to the per-user broker. A null endpoint selects the default path.
 * On failure, writes an allocated message to error when error is non-null.
 * # Safety
 * Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
 * Connection pointers must come from this library and remain open until the call returns.
 * The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
 */
struct DictationConnection *dictation_connect(const char *endpoint,
                                              char **error);

/**
 * Send a protocol Request encoded as JSON. The response or error is returned as JSON.
 * # Safety
 * Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
 * Connection pointers must come from this library and remain open until the call returns.
 * The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
 */
char *dictation_request(const struct DictationConnection *connection,
                        const char *json,
                        char **error);

/**
 * Wait for an event. Null without an error means the wait timed out.
 * # Safety
 * Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
 * Connection pointers must come from this library and remain open until the call returns.
 * The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
 */
char *dictation_next_event(const struct DictationConnection *connection,
                           uint32_t timeout_ms,
                           char **error);

/**
 * # Safety
 * The pointer must be null or an open connection returned by this library. Close each connection once; no call may use it concurrently with close.
 */
void dictation_close(struct DictationConnection *connection);

/**
 * # Safety
 * The pointer must be null or an allocated string returned by this library. Free each string exactly once.
 */
void dictation_string_free(char *value);

/**
 * Connect as a receiving application. Metadata and endpoint strings are borrowed.
 * # Safety
 * Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
 * Connection pointers must come from this library and remain open until the call returns.
 * The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
 */
struct DictationConnection *dictation_client_connect(const struct DictationApplication *application,
                                                     const char *endpoint,
                                                     char **error);

/**
 * Connect as a dictation provider. The name and endpoint strings are borrowed.
 * # Safety
 * Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
 * Connection pointers must come from this library and remain open until the call returns.
 * The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
 */
struct DictationConnection *dictation_provider_connect(const char *name,
                                                       const char *endpoint,
                                                       char **error);

#endif  /* DICTATION_API_H */
