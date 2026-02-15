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
}

impl Account {
    pub fn new(account_name: String, password: String) -> Account {
        let password_bytes = password.as_bytes();

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password_bytes, &salt)
            .unwrap()
            .to_string();

        Account {
            account_name: Some(account_name),
            password: Some(password_hash),
        }
    }

    pub fn verify_password(password: String, account_password: String) -> Result<(), AccountError> {
        println!(
            "Password: {} Account Password: {}",
            password, account_password
        );
        let password_bytes = password.as_bytes();
        let parsed_hash = PasswordHash::new(&account_password).unwrap();

        let result = Argon2::default().verify_password(password_bytes, &parsed_hash);

        match result {
            Ok(_) => Ok(()),
            Err(_e) => Err(AccountError::IncorrectPassword),
        }
    }
}
