/* A thin layer over rc_client so the Rust side only deals in strings and
 * integers, never in rcheevos's own structs. */
#include <stdio.h>
#include <string.h>

#include "rc_client.h"
#include "rc_consoles.h"
#include "rhash/md5.h"

enum {
  SLOT_RC_DONE_LOGIN = 1,
  SLOT_RC_DONE_LOAD = 2
};

extern uint32_t slot_rc_read(uint32_t address, uint8_t* buffer, uint32_t num_bytes);
extern void slot_rc_http(const char* url, const char* post_data, const char* content_type,
                         void* callback, void* callback_data);
extern void slot_rc_event(uint32_t type, const char* title, const char* description, uint32_t points);
extern void slot_rc_done(int kind, int result, const char* message);

static rc_client_t* client;

static uint32_t RC_CCONV read_memory(uint32_t address, uint8_t* buffer, uint32_t num_bytes, rc_client_t* c) {
  (void)c;
  return slot_rc_read(address, buffer, num_bytes);
}

static void RC_CCONV server_call(const rc_api_request_t* request, rc_client_server_callback_t callback,
                                 void* callback_data, rc_client_t* c) {
  (void)c;
  slot_rc_http(request->url, request->post_data, request->content_type, (void*)callback, callback_data);
}

static void RC_CCONV on_event(const rc_client_event_t* event, rc_client_t* c) {
  const char* title = "";
  const char* description = "";
  uint32_t points = 0;
  (void)c;
  if (event->achievement) {
    title = event->achievement->title ? event->achievement->title : "";
    description = event->achievement->description ? event->achievement->description : "";
    points = event->achievement->points;
  } else if (event->leaderboard) {
    title = event->leaderboard->title ? event->leaderboard->title : "";
    description = event->leaderboard->description ? event->leaderboard->description : "";
  } else if (event->server_error) {
    title = event->server_error->api ? event->server_error->api : "";
    description = event->server_error->error_message ? event->server_error->error_message : "";
  } else if (event->type == RC_CLIENT_EVENT_GAME_COMPLETED) {
    const rc_client_game_t* game = rc_client_get_game_info(client);
    title = game && game->title ? game->title : "";
  }
  slot_rc_event(event->type, title, description, points);
}

static void RC_CCONV on_login(int result, const char* message, rc_client_t* c, void* userdata) {
  (void)c;
  (void)userdata;
  slot_rc_done(SLOT_RC_DONE_LOGIN, result, message ? message : "");
}

static void RC_CCONV on_load(int result, const char* message, rc_client_t* c, void* userdata) {
  (void)c;
  (void)userdata;
  slot_rc_done(SLOT_RC_DONE_LOAD, result, message ? message : "");
}

int slot_rc_create(void) {
  if (client)
    return 1;
  client = rc_client_create(read_memory, server_call);
  if (!client)
    return 0;
  rc_client_set_event_handler(client, on_event);
  rc_client_set_hardcore_enabled(client, 0);
  rc_client_set_allow_background_memory_reads(client, 1);
  return 1;
}

void slot_rc_respond(void* callback, void* callback_data, const char* body, size_t body_length, int status) {
  rc_api_server_response_t response;
  response.body = body;
  response.body_length = body_length;
  response.http_status_code = status;
  ((rc_client_server_callback_t)callback)(&response, callback_data);
}

void slot_rc_login_password(const char* username, const char* password) {
  if (client)
    rc_client_begin_login_with_password(client, username, password, on_login, NULL);
}

void slot_rc_login_token(const char* username, const char* token) {
  if (client)
    rc_client_begin_login_with_token(client, username, token, on_login, NULL);
}

void slot_rc_logout(void) {
  if (client)
    rc_client_logout(client);
}

/* Fills user and token, returns 0 when nobody is logged in. */
int slot_rc_user(char* user, size_t user_size, char* token, size_t token_size) {
  const rc_client_user_t* info = client ? rc_client_get_user_info(client) : NULL;
  if (!info || !info->username || !info->token)
    return 0;
  strncpy(user, info->username, user_size - 1);
  user[user_size - 1] = '\0';
  strncpy(token, info->token, token_size - 1);
  token[token_size - 1] = '\0';
  return 1;
}

void slot_rc_load_game(const uint8_t* data, size_t size) {
  md5_state_t md5;
  md5_byte_t digest[16];
  char hash[33];
  int i;
  if (!client)
    return;
  md5_init(&md5);
  md5_append(&md5, data, (int)size);
  md5_finish(&md5, digest);
  for (i = 0; i < 16; i++)
    snprintf(hash + i * 2, 3, "%02x", digest[i]);
  rc_client_begin_load_game(client, hash, on_load, NULL);
}

/* Title of the loaded game and how many of its achievements are unlocked. */
int slot_rc_game(char* title, size_t title_size, uint32_t* unlocked, uint32_t* total) {
  const rc_client_game_t* game = client ? rc_client_get_game_info(client) : NULL;
  rc_client_user_game_summary_t summary;
  if (!game || !game->title)
    return 0;
  strncpy(title, game->title, title_size - 1);
  title[title_size - 1] = '\0';
  rc_client_get_user_game_summary(client, &summary);
  *unlocked = summary.num_unlocked_achievements;
  *total = summary.num_core_achievements;
  return 1;
}

void slot_rc_unload_game(void) {
  if (client)
    rc_client_unload_game(client);
}

void slot_rc_reset(void) {
  if (client)
    rc_client_reset(client);
}

void slot_rc_do_frame(void) {
  if (client)
    rc_client_do_frame(client);
}

void slot_rc_idle(void) {
  if (client)
    rc_client_idle(client);
}

size_t slot_rc_user_agent(char* buffer, size_t size) {
  return client ? rc_client_get_user_agent_clause(client, buffer, size) : 0;
}

/* Where an achievement address lives on the console's own bus. Returns how
 * many bytes from there on are in the same block, 0 if none. */
uint32_t slot_rc_real_address(uint32_t console_id, uint32_t address, uint32_t* real) {
  const rc_memory_regions_t* regions = rc_console_memory_regions(console_id);
  uint32_t i;
  if (!regions)
    return 0;
  for (i = 0; i < regions->num_regions; i++) {
    const rc_memory_region_t* r = &regions->region[i];
    if (address >= r->start_address && address <= r->end_address) {
      *real = r->real_address + (address - r->start_address);
      return r->end_address - address + 1;
    }
  }
  return 0;
}
