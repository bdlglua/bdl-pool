/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Uses CKPool's own Noise implementation to persist and validate server keys. */
#include "config.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <errno.h>
#include <sodium.h>
#include "sv2_noise.h"

static int private_file(const char *path, int allow_missing)
{
    struct stat st;
    if (lstat(path, &st)) return allow_missing && errno == ENOENT;
    return S_ISREG(st.st_mode) && st.st_uid == geteuid() && (st.st_mode & 0777) == 0600;
}

int main(int argc, char **argv)
{
    struct sv2_noise_server_keys first = {0}, second = {0};
    sv2_noise_session_t *client = NULL, *server = NULL;
    unsigned char act1[64], decoded[32], *act2 = NULL;
    size_t act2_len = 0;
    char *public_key = NULL;
    const char *error = "initialization";
    int rc = 1;
    if (argc != 3 || geteuid() == 0) {
        fprintf(stderr, "SV2_ERROR=run_as_owner_with_two_key_paths\n");
        return 1;
    }
    umask(0077);
    if (!private_file(argv[1], 1) || !private_file(argv[2], 1)) {
        fprintf(stderr, "SV2_ERROR=existing_key_file_owner_type_or_permissions\n");
        return 1;
    }
    if (!sv2_noise_load_server_keys(&first, argv[1], argv[2], 365)) goto out;
    error = "key_persistence";
    if (!private_file(argv[1], 0) || !private_file(argv[2], 0)) goto out;
    if (!sv2_noise_load_server_keys(&second, argv[1], argv[2], 365)) goto out;
    if (sodium_memcmp(first.authority_sk, second.authority_sk, 32) ||
        sodium_memcmp(first.static_sk, second.static_sk, 32)) goto out;
    error = "public_key_encoding";
    public_key = sv2_noise_authority_pubkey_b58(&second);
    if (!public_key || !sv2_noise_authority_b58_to_xonly(public_key, decoded) ||
        sodium_memcmp(decoded, second.authority_xonly, 32)) goto out;
    error = "noise_self_handshake";
    server = sv2_noise_session_new(&second);
    client = sv2_noise_client_session_new(second.authority_xonly);
    if (!server || !client || !sv2_noise_client_act1(client, act1)) goto out;
    if (!sv2_noise_handshake_read(server, act1, sizeof(act1), &act2, &act2_len)) goto out;
    if (!sv2_noise_client_act2(client, act2, act2_len) ||
        !sv2_noise_handshake_complete(server) || !sv2_noise_handshake_complete(client)) goto out;
    printf("SV2_AUTHORITY_PUBLIC_KEY=%s\n", public_key);
    rc = 0;
out:
    if (rc) fprintf(stderr, "SV2_ERROR=%s\n", error);
    free(act2);
    free(public_key);
    if (client) sv2_noise_session_free(client);
    if (server) sv2_noise_session_free(server);
    sv2_noise_server_keys_clear(&first);
    sv2_noise_server_keys_clear(&second);
    return rc;
}
