#include "dictation_api.h"
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <process.h>
#define process_id _getpid
#else
#include <unistd.h>
#define process_id getpid
#endif
int main(int argc, char **argv) {
  if (argc != 2) return 2;
  char *error = NULL;
  DictationApplication app = {"c.compiled", "Compiled C receiver", NULL, (uint32_t)process_id()};
  DictationConnection *client = dictation_client_connect(&app, argv[1], &error);
  if (!client) { fprintf(stderr, "%s\n", error); dictation_string_free(error); return 1; }
  DictationConnection *provider = dictation_provider_connect("Compiled C provider", argv[1], &error);
  if (!provider) { dictation_close(client); dictation_string_free(error); return 1; }
  char *reply = dictation_request(client, "{\"op\":\"activate\"}", &error);
  int success = reply && strstr(reply, "activated") && !error;
  dictation_string_free(reply); dictation_string_free(error);
  dictation_close(provider); dictation_close(client);
  return success ? 0 : 1;
}
