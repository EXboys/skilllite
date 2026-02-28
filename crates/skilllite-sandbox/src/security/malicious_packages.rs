//! Offline malicious / typosquatting package name library (B4).
//!
//! A curated, statically embedded list of known-malicious and typosquatting
//! package names for PyPI and npm.  No network call required — just a sorted
//! slice + binary search at the time of install.
//!
//! Sources: documented PyPI removal reports (2018-2024), npm security
//! advisories, GitHub Advisory Database, Snyk / Socket.dev disclosures.
//!
//! # Memory cost
//! Static string data only — binary size increase is proportional to the total
//! length of the strings, roughly **20-60 KB** for this list.
//!
//! # Usage
//! ```rust,ignore
//! use skilllite_sandbox::security::malicious_packages::check_malicious_package;
//!
//! if let Some(reason) = check_malicious_package("colourama", "PyPI") {
//!     eprintln!("Blocked: {}", reason);
//! }
//! ```

// Each entry: (lowercase_package_name, human_readable_reason).
// Sorted ascending by name for binary search — DO NOT re-order manually;
// run `cargo test` to verify the invariant.

/// Known-malicious or typosquatting PyPI package names (lowercase).
/// MUST be sorted ascending by name — verified by unit test.
static MALICIOUS_PYPI: &[(&str, &str)] = &[
    ("aiohttp2",         "Fake aiohttp package"),
    ("aiounittest2",     "Fake aiounittest package"),
    ("alive-bar",        "Typosquat of alive-progress"),
    ("amazons3",         "Fake AWS S3 package"),
    ("awscli2",          "Fake awscli package"),
    ("beautifulsoup3",   "Fake beautifulsoup4 package"),
    ("bnc-iac-scan",     "Documented supply chain malware (2023)"),
    ("bota3",            "Typosquat of boto3"),
    ("bs4-python",       "Fake beautifulsoup4 package"),
    ("bto3",             "Typosquat of boto3"),
    ("ccxt2",            "Fake ccxt cryptocurrency trading library"),
    ("celery2",          "Fake celery package"),
    ("click2",           "Fake click CLI package"),
    ("coloredlogs2",     "Fake coloredlogs package"),
    ("colourama",        "Typosquat of colorama — documented 2018 malware"),
    ("cryptography2",    "Fake cryptography package"),
    ("cryptograpy",      "Typosquat of cryptography"),
    ("crytography",      "Typosquat of cryptography"),
    ("ctx",              "Supply chain attack (2022): exfiltrated env vars to remote server"),
    ("diango",           "Typosquat of django"),
    ("discord-rad",      "Malicious Discord package"),
    ("discord-self",     "Malicious Discord selfbot — credential theft"),
    ("discord-selfbot",  "Malicious Discord selfbot — credential theft"),
    ("discordclient",    "Malicious Discord client library"),
    ("djang",            "Typosquat of django"),
    ("django2",          "Fake django package"),
    ("djangoo",          "Typosquat of django"),
    ("dpp",              "Supply chain attack (2022): companion malware to ctx"),
    ("eth-account2",     "Fake eth-account package"),
    ("exotel",           "Documented credential stealer (2022)"),
    ("faker2",           "Fake faker package"),
    ("falsk",            "Typosquat of flask"),
    ("fastapi2",         "Fake FastAPI package"),
    ("flaask",           "Typosquat of flask"),
    ("grpcio2",          "Fake grpcio package"),
    ("httplib2-python",  "Fake httplib2 package"),
    ("httpx2",           "Fake httpx package"),
    ("loguru-colorize",  "Fake loguru variant"),
    ("loguru2",          "Fake loguru package"),
    ("macos-utils",      "Documented macOS credential stealer (2023)"),
    ("netstat-ng",       "Documented malware: network reconnaissance tool"),
    ("numpy2",           "Fake numpy package"),
    ("numpyl",           "Typosquat of numpy"),
    ("nunpy",            "Typosquat of numpy"),
    ("openssl-python",   "Fake OpenSSL package"),
    ("panads",           "Typosquat of pandas"),
    ("pandas2",          "Fake pandas package"),
    ("pandaz",           "Typosquat of pandas"),
    ("paramikoo",        "Typosquat of paramiko"),
    ("paramuko",         "Typosquat of paramiko"),
    ("pillow-py",        "Fake Pillow package"),
    ("pillow2",          "Fake Pillow package"),
    ("pilow",            "Typosquat of Pillow"),
    ("pycrypto2",        "Fake pycryptodome package"),
    ("pycryptodome2",    "Fake pycryptodome package"),
    ("pymongo2",         "Fake pymongo package"),
    ("pynput2",          "Fake pynput package"),
    ("pyopenssl2",       "Fake PyOpenSSL package"),
    ("pyperclip2",       "Fake pyperclip package"),
    ("pyperclipboard",   "Fake pyperclip package"),
    ("pytest2",          "Fake pytest package"),
    ("python-binance2",  "Fake python-binance package"),
    ("python-dateutil2", "Fake python-dateutil package"),
    ("python-decouple2", "Fake python-decouple package"),
    ("python-ftp",       "Fake FTP package shadowing ftplib"),
    ("python-nmap2",     "Fake python-nmap package"),
    ("python-requests2", "Fake requests package"),
    ("python-sqlite3",   "Fake sqlite3 package — shadows stdlib"),
    ("python-utils2",    "Fake python-utils package"),
    ("python-whois2",    "Fake python-whois package"),
    ("python3-dateutil", "Typosquat of python-dateutil — documented attack"),
    ("redis2",           "Fake redis-py package"),
    ("reqeusts",         "Typosquat of requests (transposed letters)"),
    ("requests-html2",   "Fake requests-html package"),
    ("requestz",         "Typosquat of requests"),
    ("rich2",            "Fake rich package"),
    ("scikit-learn2",    "Fake scikit-learn package"),
    ("setup-tools",      "Typosquat of setuptools"),
    ("setupool",         "Typosquat of setuptools"),
    ("setuptool",        "Typosquat of setuptools"),
    ("shell-exec",       "Suspicious package — known malware delivery category"),
    ("sklearn2",         "Fake scikit-learn package"),
    ("sqlalchemy2",      "Fake sqlalchemy package"),
    ("starlette2",       "Fake starlette package"),
    ("tensorflow2-cpu",  "Fake TensorFlow package"),
    ("tqdm2",            "Fake tqdm package"),
    ("urlib3",           "Typosquat of urllib3"),
    ("urllib",           "Shadows Python stdlib urllib; known typosquat vector"),
    ("urllib2",          "Python 2 package repackaged maliciously"),
    ("uvicorn2",         "Fake uvicorn package"),
    ("web3-ethereum",    "Fake web3 package"),
    ("websockets2",      "Fake websockets package"),
];

/// Known-malicious or typosquatting npm package names (lowercase).
/// MUST be sorted ascending by name — verified by unit test.
static MALICIOUS_NPM: &[(&str, &str)] = &[
    ("@marak/colors.js",           "Author-sabotaged protest package (2022)"),
    ("@xaop/xaop",                 "Known cryptominer delivery package"),
    ("axios-fetch",                "Fake axios package"),
    ("axois",                      "Typosquat of axios"),
    ("crypto-js-aes",              "Fake crypto-js package"),
    ("discord-selfbot-v13",        "Malicious Discord selfbot — account theft"),
    ("discord.js-selfbot-v13",     "Malicious Discord selfbot — account theft"),
    ("discordjs",                  "Typosquat / fake discord.js"),
    ("electron-native-notify",     "Compromised: cryptominer injected (2018)"),
    ("event-stream",               "Compromised 2018: flatmap-stream injected to steal Bitcoin"),
    ("express-fileupload-plus",    "Fake express-fileupload package"),
    ("flatmap-stream",             "Companion malware to event-stream attack (2018)"),
    ("install-shelljs",            "Suspicious: installs shell execution capability"),
    ("lodahs",                     "Typosquat of lodash"),
    ("lodash2",                    "Fake lodash package"),
    ("momnet",                     "Typosquat of moment"),
    ("node-ipc",                   "Political wiperware injected in v10.1.1-v10.1.2 (2022)"),
    ("node-shell",                 "Suspicious: wraps arbitrary shell execution"),
    ("nodemailer-js",              "Fake nodemailer package"),
    ("react-dom2",                 "Fake react-dom package"),
    ("react2",                     "Fake react package"),
    ("ua-parser-js",               "Hijacked 2021: cryptominer + credential stealer injected"),
    ("vue2-cli",                   "Fake @vue/cli package"),
    ("webpack2",                   "Fake webpack package"),
];

// ─── Public API ──────────────────────────────────────────────────────────────

/// Result of an offline malicious-package check.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MaliciousPackageHit {
    /// Package name as found in the dependency file.
    pub name: String,
    /// Ecosystem: "PyPI" or "npm".
    pub ecosystem: String,
    /// Human-readable reason from the embedded database.
    pub reason: &'static str,
}

/// Check a single package against the offline malicious-package database.
///
/// Returns `Some(MaliciousPackageHit)` if the package is in the database,
/// `None` if it is not known to be malicious.
///
/// Comparison is **case-insensitive** — the name is lowercased before lookup.
pub fn check_malicious_package(name: &str, ecosystem: &str) -> Option<MaliciousPackageHit> {
    let lower = name.to_lowercase();
    let table: &[(&str, &str)] = match ecosystem {
        "PyPI" => MALICIOUS_PYPI,
        "npm" => MALICIOUS_NPM,
        _ => return None,
    };
    // Binary search — tables are sorted ascending by name
    table
        .binary_search_by_key(&lower.as_str(), |(pkg, _)| pkg)
        .ok()
        .map(|idx| MaliciousPackageHit {
            name: name.to_string(),
            ecosystem: ecosystem.to_string(),
            reason: table[idx].1,
        })
}

/// Check a list of `(name, ecosystem)` pairs and return all hits.
///
/// This is the batch variant used by the dependency auditor.
pub fn check_malicious_packages<'a>(
    packages: impl Iterator<Item = (&'a str, &'a str)>,
) -> Vec<MaliciousPackageHit> {
    packages
        .filter_map(|(name, eco)| check_malicious_package(name, eco))
        .collect()
}

// ─── Invariant test ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pypi_table_is_sorted() {
        let names: Vec<&str> = MALICIOUS_PYPI.iter().map(|(n, _)| *n).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "MALICIOUS_PYPI must be sorted ascending by name");
    }

    #[test]
    fn npm_table_is_sorted() {
        let names: Vec<&str> = MALICIOUS_NPM.iter().map(|(n, _)| *n).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "MALICIOUS_NPM must be sorted ascending by name");
    }

    #[test]
    fn known_malicious_pypi_detected() {
        let hit = check_malicious_package("colourama", "PyPI").unwrap();
        assert!(hit.reason.contains("colorama"));
        let hit2 = check_malicious_package("Colourama", "PyPI").unwrap(); // case insensitive
        assert_eq!(hit.name, "colourama");
        assert_eq!(hit2.name, "Colourama");
    }

    #[test]
    fn known_malicious_npm_detected() {
        let hit = check_malicious_package("event-stream", "npm").unwrap();
        assert!(hit.reason.contains("flatmap-stream"));
    }

    #[test]
    fn clean_package_not_detected() {
        assert!(check_malicious_package("requests", "PyPI").is_none());
        assert!(check_malicious_package("lodash", "npm").is_none());
    }
}


