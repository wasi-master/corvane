//! Access tokens live in the OS keychain (macOS Keychain via `keyring`),
//! one item per (host, login), like GitHub Desktop's token store.

use tracing::debug;

const SERVICE: &str = crate::BUNDLE_ID;

#[derive(Debug, thiserror::Error)]
pub enum KeychainError {
    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),
}

pub type Result<T> = std::result::Result<T, KeychainError>;

fn entry(host: &str, login: &str) -> Result<keyring::Entry> {
    // Account column shows as "login@host" in Keychain Access.
    Ok(keyring::Entry::new(SERVICE, &format!("{login}@{host}"))?)
}

pub fn store_token(host: &str, login: &str, token: &str) -> Result<()> {
    debug!(host, login, "storing token in keychain");
    entry(host, login)?.set_password(token)?;
    Ok(())
}

pub fn token(host: &str, login: &str) -> Result<Option<String>> {
    match entry(host, login)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// Generic git server credentials (GHD `setGenericPassword`): one item per
/// (host, username), separate from the GitHub token entries.
fn generic_entry(host: &str, username: &str) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(
        SERVICE,
        &format!("git:{username}@{host}"),
    )?)
}

pub fn store_generic_password(host: &str, username: &str, password: &str) -> Result<()> {
    debug!(host, username, "storing generic git password in keychain");
    generic_entry(host, username)?.set_password(password)?;
    Ok(())
}

pub fn generic_password(host: &str, username: &str) -> Result<Option<String>> {
    match generic_entry(host, username)?.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn delete_token(host: &str, login: &str) -> Result<()> {
    match entry(host, login)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// A GitHub Enterprise OAuth app's client secret (the browser flow's token
/// exchange), one item per (host, client ID).
fn oauth_secret_entry(host: &str, client_id: &str) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(
        SERVICE,
        &format!("oauth:{client_id}@{host}"),
    )?)
}

pub fn store_oauth_client_secret(host: &str, client_id: &str, secret: &str) -> Result<()> {
    debug!(host, client_id, "storing OAuth client secret in keychain");
    oauth_secret_entry(host, client_id)?.set_password(secret)?;
    Ok(())
}

pub fn oauth_client_secret(host: &str, client_id: &str) -> Result<Option<String>> {
    match oauth_secret_entry(host, client_id)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn delete_oauth_client_secret(host: &str, client_id: &str) -> Result<()> {
    match oauth_secret_entry(host, client_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err.into()),
    }
}
