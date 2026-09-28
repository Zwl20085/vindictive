//! GitHub token storage in the OS credential store (Windows Credential
//! Manager). The token never touches the settings file or the log.

const SERVICE: &str = "vindictive";
const USER: &str = "github-token";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, USER).map_err(|e| format!("credential store unavailable: {e}"))
}

pub fn get_token() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(t) if t.trim().is_empty() => Ok(None),
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("could not read token: {e}")),
    }
}

pub fn set_token(token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("token is empty".into());
    }
    if token.chars().any(|c| c.is_control()) {
        return Err("token contains invalid characters".into());
    }
    entry()?
        .set_password(token)
        .map_err(|e| format!("could not store token: {e}"))
}

pub fn clear_token() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("could not delete token: {e}")),
    }
}
