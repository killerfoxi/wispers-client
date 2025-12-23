#include "wispers_connect.h"
#include <stdio.h>
#include <string.h>

int main(void) {
    WispersNodeStorageHandle *storage = wispers_storage_new_in_memory();
    if (!storage) {
        fprintf(stderr, "failed to init storage\n");
        return 1;
    }

    WispersPendingNodeStateHandle *pending = NULL;
    WispersRegisteredNodeStateHandle *registered = NULL;
    WispersStatus status = wispers_storage_restore_or_init(
        storage,
        "app.example",
        NULL,
        &pending,
        &registered
    );

    if (status != WISPERS_STATUS_SUCCESS) {
        fprintf(stderr, "restore/init failed: %d\n", status);
        wispers_storage_free(storage);
        return 1;
    }

    if (registered) {
        printf("already registered\n");
        wispers_registered_state_free(registered);
    } else if (pending) {
        char *url = wispers_pending_state_registration_url(pending, "https://wispers.dev/add-node");
        printf("Registration URL: %s\n", url);
        wispers_string_free(url);

        status = wispers_pending_state_complete_registration(
            pending,
            "connectivity-group",
            "node-123",
            &registered
        );

        if (status != WISPERS_STATUS_SUCCESS) {
            fprintf(stderr, "complete_registration failed: %d\n", status);
            wispers_storage_free(storage);
            return 1;
        }

        printf("Registration complete!\n");
        wispers_registered_state_free(registered);
    }

    wispers_storage_free(storage);
    return 0;
}
