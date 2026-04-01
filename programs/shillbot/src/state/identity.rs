use anchor_lang::prelude::*;

/// On-chain binding between an agent wallet and a platform identity.
/// PDA seeds: `["identity", agent.key().as_ref(), &[platform]]`
///
/// One identity per (agent, platform) pair. Agent must revoke before
/// re-registering with a different platform user ID.
#[account]
pub struct PlatformIdentity {
    /// The agent wallet that owns this identity.
    pub agent: Pubkey,
    /// Platform type (PlatformType discriminant).
    pub platform: u8,
    /// SHA-256 hash of the platform user ID (e.g., X handle or user ID string).
    /// Hashed to save space — the verifier knows the plaintext and can verify the hash.
    pub identity_hash: [u8; 32],
    /// Unix timestamp of registration.
    pub registered_at: i64,
    pub bump: u8,
}

impl PlatformIdentity {
    // 8 + 32 + 1 + 32 + 8 + 1 = 82
    pub const SPACE: usize = 8 // discriminator
        + 32  // agent
        + 1   // platform
        + 32  // identity_hash
        + 8   // registered_at
        + 1; // bump
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_identity_space_is_82() {
        assert_eq!(PlatformIdentity::SPACE, 82);
    }
}
