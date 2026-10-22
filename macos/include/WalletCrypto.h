#include <stdint.h>
#include <stddef.h>
void mw_zeroize(uint8_t *buffer, size_t len);
int32_t mw_mnemonic_from_entropy(const uint8_t entropy[16], uint8_t mnemonic_out[256]);
int32_t mw_ethereum_address(const uint8_t public_key[65], uint8_t address_out[20]);
int32_t mw_derive(uint32_t curve, const uint8_t *mnemonic, size_t mnemonic_len,
                  const uint8_t *passphrase, size_t passphrase_len,
                  const uint8_t *path, size_t path_len,
                  uint8_t key_out[32], uint8_t public_out[65]);
