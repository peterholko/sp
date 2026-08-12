use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Account {
    pub account_name: Option<String>,
    pub password: Option<String>,
}

#[derive(Error, Debug)]
pub enum AccountError {
    #[error("Incorrect Password")]
    IncorrectPassword,
    #[error("Unable to hash password")]
    PasswordHashFailure,
}

impl Account {
    pub fn new(account_name: String, password: String) -> Result<Account, AccountError> {
        Ok(Account {
            account_name: Some(account_name),
            password: Some(Account::hash_password(&password)?),
        })
    }

    /// Argon2-hash a password (salted). Used by registration and password reset.
    pub fn hash_password(password: &str) -> Result<String, AccountError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| AccountError::PasswordHashFailure)
    }

    pub fn verify_password(password: &str, account_password: &str) -> Result<(), AccountError> {
        let password_bytes = password.as_bytes();
        let parsed_hash =
            PasswordHash::new(account_password).map_err(|_| AccountError::IncorrectPassword)?;

        let result = Argon2::default().verify_password(password_bytes, &parsed_hash);

        match result {
            Ok(_) => Ok(()),
            Err(_e) => Err(AccountError::IncorrectPassword),
        }
    }
}
