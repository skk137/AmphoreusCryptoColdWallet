use zeroize::{Zeroize, ZeroizeOnDrop};

/// Wraps secret bytes (seed material, private keys) so they are wiped from
/// memory on drop. Deliberately does not implement `Debug`/`Display` — that
/// is the compile-time guardrail against ever accidentally logging a secret.
#[derive(Clone, ZeroizeOnDrop)]
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Same as `SecretBytes` but for a UTF-8 phrase (the BIP39 mnemonic itself).
#[derive(Clone, ZeroizeOnDrop)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Explicit helper for zeroizing a stack buffer once it's no longer needed,
/// used for entropy/seed scratch space that isn't wrapped in the types above.
pub fn wipe(buf: &mut [u8]) {
    buf.zeroize();
}
