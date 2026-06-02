#ifndef SCHILDPAD_H
#define SCHILDPAD_H

#include <stdint.h>
#include <stdbool.h>

/* Opaque handle to a schildpad-core Engine. */
typedef struct SchildpadEngine SchildpadEngine;

/* Lifecycle. */
SchildpadEngine *schildpad_new(void);
void schildpad_free(SchildpadEngine *engine);

/* Host-supplied render target (the child never sets this). width_px is always cols*8. */
void schildpad_set_render_target(SchildpadEngine *engine, uint16_t cols, uint16_t rows);
void schildpad_reset_seed(SchildpadEngine *engine, uint64_t seed);

/* All of these return a heap-allocated JSON array of events as a C string.
   The caller MUST free it with schildpad_string_free. */
char *schildpad_load(SchildpadEngine *engine, const char *src);
char *schildpad_reset(SchildpadEngine *engine);
char *schildpad_step(SchildpadEngine *engine);
char *schildpad_run_line(SchildpadEngine *engine, const char *src, uint32_t line);

/* JSON array of the live turtle sprite snapshot. Free with schildpad_string_free. */
char *schildpad_sprites(SchildpadEngine *engine);

bool schildpad_done(SchildpadEngine *engine);
int32_t schildpad_current_line(SchildpadEngine *engine); /* -1 if none */

/* Stateless syntax highlighting: classify every token in src into colour kinds.
   Returns a JSON array [{"line":1,"col":0,"len":4,"kind":"keyword","ok":true}, ...].
   Free with schildpad_string_free. No engine handle needed. */
char *schildpad_highlight(const char *src);

void schildpad_string_free(char *s);

#endif /* SCHILDPAD_H */
