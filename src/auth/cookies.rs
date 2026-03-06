use anyhow::{bail, Context, Result};
use std::path::Path;

/// A parsed Netscape cookie entry
#[derive(Debug)]
#[allow(dead_code)]
pub struct Cookie {
    pub domain: String,
    pub name: String,
    pub value: String,
}

/// Parse cookies from Netscape cookie.txt format
/// Lines starting with # are comments, empty lines are skipped
/// Fields: domain, flag, path, secure, expiry, name, value
pub fn parse_netscape_cookies(content: &str) -> Result<Vec<Cookie>> {
    let mut cookies = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 7 {
            continue; // Skip malformed lines
        }
        cookies.push(Cookie {
            domain: fields[0].to_string(),
            name: fields[5].to_string(),
            value: fields[6].to_string(),
        });
    }
    if cookies.is_empty() {
        bail!("no cookies found in file");
    }
    Ok(cookies)
}

/// Filter a Netscape cookie file to only .youtube.com domain lines.
/// RustyPipe's parser requires exactly `.youtube.com` domain and errors
/// on malformed lines, so we must pre-filter strictly.
fn filter_youtube_cookies(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let fields: Vec<&str> = trimmed.split('\t').collect();
        if fields.len() >= 7 && fields[0] == ".youtube.com" {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Import a cookie file: filter to YouTube/Google, save to destination with restricted permissions
pub fn import_cookie_file(source: &Path, dest: &Path) -> Result<()> {
    // Validate source exists
    if !source.exists() {
        bail!("cookie file not found: {}", source.display());
    }

    // Read, filter to YouTube domains, and validate
    let content = std::fs::read_to_string(source).context("failed to read cookie file")?;
    let filtered = filter_youtube_cookies(&content);
    let _ = parse_netscape_cookies(&filtered)?;

    // Create destination directory
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).context("failed to create cookie directory")?;
    }

    // Write filtered cookies
    std::fs::write(dest, &filtered).context("failed to write cookie file")?;

    // Set permissions to 0600 (owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o600))
            .context("failed to set cookie file permissions")?;
    }

    Ok(())
}

/// Check if a cookie file at the given path is valid
pub fn validate_cookies(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    match std::fs::read_to_string(path) {
        Ok(content) => parse_netscape_cookies(&content).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const SAMPLE_COOKIES: &str = r#"# Netscape HTTP Cookie File
.youtube.com	TRUE	/	TRUE	1700000000	SID	abc123
.youtube.com	TRUE	/	TRUE	1700000000	HSID	def456
.youtube.com	TRUE	/	TRUE	1700000000	SSID	ghi789
"#;

    #[test]
    fn test_parse_netscape_cookies() {
        let cookies = parse_netscape_cookies(SAMPLE_COOKIES).unwrap();
        assert_eq!(cookies.len(), 3);
        assert_eq!(cookies[0].domain, ".youtube.com");
        assert_eq!(cookies[0].name, "SID");
        assert_eq!(cookies[0].value, "abc123");
    }

    #[test]
    fn test_parse_empty_cookies() {
        let result = parse_netscape_cookies("# just comments\n\n");
        assert!(result.is_err());
    }

    #[test]
    fn test_import_cookie_file() {
        let mut source = NamedTempFile::new().unwrap();
        write!(source, "{}", SAMPLE_COOKIES).unwrap();

        let dest_dir = tempfile::tempdir().unwrap();
        let dest_path = dest_dir.path().join("session").join("cookies.txt");

        import_cookie_file(source.path(), &dest_path).unwrap();

        assert!(dest_path.exists());
        let content = std::fs::read_to_string(&dest_path).unwrap();
        assert!(content.contains("SID"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(&dest_path).unwrap().permissions();
            assert_eq!(perms.mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn test_validate_cookies() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", SAMPLE_COOKIES).unwrap();
        assert!(validate_cookies(f.path()));

        let mut bad = NamedTempFile::new().unwrap();
        write!(bad, "not a cookie file").unwrap();
        assert!(!validate_cookies(bad.path()));
    }
}
