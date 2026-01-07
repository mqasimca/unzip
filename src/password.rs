//! Password handling for encrypted ZIP archives
//!
//! Provides secure password input functionality with interactive prompts
//! and validation for encrypted archive extraction.

use anyhow::{Context, Result};

/// Get password for encrypted archive
///
/// If a password is provided via command line (-P), use it (with a warning about security).
/// Otherwise, prompt the user interactively with no echo.
///
/// # Arguments
///
/// * `password_arg` - Optional password from command line
/// * `quiet` - Quiet level (0 = normal, 1 = quiet, 2 = very quiet)
///
/// # Returns
///
/// Returns the password as a byte vector, or None if no password is needed
///
/// # Errors
///
/// Returns an error if password input fails
pub fn get_password(password_arg: Option<&str>, quiet: u8) -> Result<Option<Vec<u8>>> {
    if let Some(pwd) = password_arg {
        if quiet == 0 {
            eprintln!("Warning: Using -P option is insecure. Password is visible in process list.");
            eprintln!(
                "Consider using interactive password prompt instead (just press Enter when prompted)."
            );
        }
        return Ok(Some(pwd.as_bytes().to_vec()));
    }

    // No password provided - don't prompt unless we encounter an encrypted file
    Ok(None)
}

/// Prompt user for password interactively
///
/// This function prompts the user to enter a password with no echo.
/// The password is returned as a byte vector.
///
/// # Returns
///
/// Returns the password entered by the user
///
/// # Errors
///
/// Returns an error if password reading fails
pub fn prompt_for_password() -> Result<Vec<u8>> {
    let password = rpassword::prompt_password("Enter password for encrypted files: ")
        .context("Failed to read password")?;

    Ok(password.into_bytes())
}

/// Check if a ZIP error indicates an encrypted file that needs a password
///
/// # Arguments
///
/// * `error` - The error message to check
///
/// # Returns
///
/// Returns true if the error indicates password is needed
pub fn is_password_error(error: &str) -> bool {
    error.contains("password")
        || error.contains("encrypted")
        || error.contains("InvalidPassword")
        || error.contains("UnsupportedArchive")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_password_from_arg() {
        let result = get_password(Some("testpass"), 2).unwrap();
        assert_eq!(result, Some(b"testpass".to_vec()));
    }

    #[test]
    fn test_get_password_no_arg() {
        let result = get_password(None, 2).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_is_password_error() {
        assert!(is_password_error("Invalid password provided"));
        assert!(is_password_error("File is encrypted"));
        assert!(is_password_error("InvalidPassword"));
        assert!(is_password_error("UnsupportedArchive"));
        assert!(!is_password_error("File not found"));
    }
}
