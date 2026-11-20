/*
 * PROPRIETARY — Kshana Pro overlay. See LICENSE.
 *
 * KS-Inside minimal example: open with a licence token from a file, run one drift check on
 * a short time-error series, print the answer, close.
 *
 *   cc -I sdk/ks-inside/include sdk/ks-inside/examples/minimal.c \
 *      -L <dir with libkshana_pro.so> -lkshana_pro -o ks-inside-minimal
 *   ./ks-inside-minimal oem.token SN-0001 meter.jsonl
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ks_inside.h"

static char *read_file(const char *path) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *buf = malloc((size_t)n + 1);
    if (buf && fread(buf, 1, (size_t)n, f) != (size_t)n) { free(buf); buf = NULL; }
    if (buf) {
        buf[n] = '\0';
        while (n > 0 && (buf[n - 1] == '\n' || buf[n - 1] == '\r')) buf[--n] = '\0';
    }
    fclose(f);
    return buf;
}

int main(int argc, char **argv) {
    if (argc != 4) {
        fprintf(stderr, "usage: %s <token-file> <unit-id> <meter-log>\n", argv[0]);
        return 2;
    }
    if (ks_inside_abi_version() != KS_INSIDE_ABI_VERSION) {
        fprintf(stderr, "library ABI %u, header %d\n", ks_inside_abi_version(), KS_INSIDE_ABI_VERSION);
        return 2;
    }
    char *token = read_file(argv[1]);
    if (!token) { perror(argv[1]); return 2; }

    ks_inside *h = NULL;
    char *err = NULL;
    int32_t rc = ks_inside_open(token, argv[2], argv[3], NULL, &h, &err);
    free(token);
    if (rc != KS_OK) {
        fprintf(stderr, "open failed (%d): %s\n", rc, err ? err : "");
        ks_inside_free_string(err);
        return 1;
    }

    /* A clock that holds, then starts to walk off at 5 ns/s. */
    char input[65536];
    size_t used = (size_t)snprintf(input, sizeof input, "{\"series\":[");
    for (int i = 0; i < 900 && used < sizeof input - 64; i++) {
        double te = i < 400 ? 0.0 : (i - 400) * 5e-9;
        used += (size_t)snprintf(input + used, sizeof input - used, "%s[%d,%.3e]",
                                 i ? "," : "", 1800000000 + i, te);
    }
    snprintf(input + used, sizeof input - used, "],\"name\":\"example\"}");

    char *out = NULL;
    rc = ks_inside_call(h, "timing.drift", input, &out);
    printf("%s\n", out ? out : "");
    ks_inside_free_string(out);
    ks_inside_close(h);
    return rc == KS_OK ? 0 : 1;
}
