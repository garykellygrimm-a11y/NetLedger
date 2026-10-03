use std::{collections::HashSet, sync::LazyLock};

use anyhow::{Result, anyhow};
use argon2::{
    Argon2,
    password_hash::{self, PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use pbkdf2::Pbkdf2;
use unicode_normalization::UnicodeNormalization;

pub const MIN_PASSWORD_CHARS: usize = 15;
pub const MAX_PASSWORD_CHARS: usize = 256;

static COMMON_PASSWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    include_str!("../data/common-passwords.txt")
        .lines()
        .filter(|line| !line.is_empty())
        .collect()
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashAlgorithm {
    Argon2id,
    Pbkdf2Sha256,
}

impl HashAlgorithm {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "argon2id" => Some(Self::Argon2id),
            "pbkdf2-sha256" => Some(Self::Pbkdf2Sha256),
            _ => None,
        }
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by sign-in in the next checkpoint")
    )]
    fn phc_id(self) -> &'static str {
        match self {
            Self::Argon2id => "argon2id",
            Self::Pbkdf2Sha256 => "pbkdf2-sha256",
        }
    }
}

fn normalize(password: &str) -> String {
    password.nfc().collect()
}

pub fn check_new_password(password: &str, username: &str) -> Result<(), String> {
    let normalized = normalize(password);
    let length = normalized.chars().count();

    if length < MIN_PASSWORD_CHARS {
        return Err(format!(
            "password must be at least {MIN_PASSWORD_CHARS} characters"
        ));
    }
    if length > MAX_PASSWORD_CHARS {
        return Err(format!(
            "password must be at most {MAX_PASSWORD_CHARS} characters"
        ));
    }

    let lowered = normalized.to_lowercase();
    if COMMON_PASSWORDS.contains(lowered.as_str()) {
        return Err("password is too common; choose a different one".to_string());
    }
    if lowered.contains("netledger") || (!username.is_empty() && lowered.contains(username)) {
        return Err("password must not contain the username or the product name".to_string());
    }

    Ok(())
}

pub fn hash_password(password: &str, algorithm: HashAlgorithm) -> Result<String> {
    let normalized = normalize(password);
    let result: password_hash::Result<PasswordHash> = match algorithm {
        HashAlgorithm::Argon2id => Argon2::default().hash_password(normalized.as_bytes()),
        HashAlgorithm::Pbkdf2Sha256 => Pbkdf2::default().hash_password(normalized.as_bytes()),
    };
    let hash = result.map_err(|err| anyhow!("failed to hash password: {err}"))?;

    Ok(hash.to_string())
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by sign-in in the next checkpoint")
)]
pub fn verify_password(password: &str, stored: &str) -> bool {
    let Ok(hash) = PasswordHash::new(stored) else {
        return false;
    };
    let normalized = normalize(password);

    let result = match hash.algorithm.as_str() {
        "argon2id" => Argon2::default().verify_password(normalized.as_bytes(), &hash),
        "pbkdf2-sha256" => Pbkdf2::default().verify_password(normalized.as_bytes(), &hash),
        _ => return false,
    };

    result.is_ok()
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by sign-in in the next checkpoint")
)]
pub fn needs_rehash(stored: &str, algorithm: HashAlgorithm) -> bool {
    PasswordHash::new(stored)
        .map(|hash| hash.algorithm.as_str() != algorithm.phc_id())
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "correct horse battery staple";

    #[test]
    fn argon2id_hashes_verify() {
        let hash = hash_password(GOOD, HashAlgorithm::Argon2id).unwrap();

        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password(GOOD, &hash));
        assert!(!verify_password("wrong horse battery staple", &hash));
    }

    #[test]
    fn pbkdf2_hashes_verify() {
        let hash = hash_password(GOOD, HashAlgorithm::Pbkdf2Sha256).unwrap();

        assert!(hash.starts_with("$pbkdf2-sha256$"));
        assert!(verify_password(GOOD, &hash));
        assert!(!verify_password("wrong horse battery staple", &hash));
    }

    #[test]
    fn each_hash_uses_a_new_salt() {
        let first = hash_password(GOOD, HashAlgorithm::Argon2id).unwrap();
        let second = hash_password(GOOD, HashAlgorithm::Argon2id).unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn unicode_forms_of_the_same_password_match() {
        let composed = "caf\u{e9} au lait every morning";
        let decomposed = "cafe\u{301} au lait every morning";
        let hash = hash_password(composed, HashAlgorithm::Argon2id).unwrap();

        assert!(verify_password(decomposed, &hash));
    }

    #[test]
    fn malformed_hashes_never_verify() {
        assert!(!verify_password(GOOD, "not a hash"));
        assert!(!verify_password(GOOD, ""));
    }

    #[test]
    fn rehash_is_needed_when_the_algorithm_changes() {
        let hash = hash_password(GOOD, HashAlgorithm::Argon2id).unwrap();

        assert!(!needs_rehash(&hash, HashAlgorithm::Argon2id));
        assert!(needs_rehash(&hash, HashAlgorithm::Pbkdf2Sha256));
    }

    #[test]
    fn new_passwords_follow_the_policy() {
        assert!(check_new_password(GOOD, "gary").is_ok());
        assert!(check_new_password("short", "gary").is_err());
        assert!(check_new_password(&"a".repeat(257), "gary").is_err());
        assert!(check_new_password("1q2w3e4r5t6y7u8i", "gary").is_err());
        assert!(check_new_password("gary has a long password", "gary").is_err());
        assert!(check_new_password("my netledger password is long", "gary").is_err());
    }

    #[test]
    fn no_composition_rules_are_imposed() {
        assert!(check_new_password("all lowercase words and spaces", "gary").is_ok());
    }
}
