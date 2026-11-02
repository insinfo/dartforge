//! `dart pub token` (`add`, `list`, `remove`), `logout` and `uploader`, as in
//! Pub 3.6.2: secret tokens in `<dart config dir>/pub-tokens.json` and the
//! OAuth2 credentials file `pub-credentials.json` next to it.
use super::{
    Arguments, COMMON, Failure, Spec, data, flag, option,
    tree::{Node, parse_json},
    usage,
};
use std::{
    io::{BufRead, Write},
    path::PathBuf,
};

const FOOTER: &str = "Run \"dart help\" to see global options.";
const DOCS: &str = "See https://dart.dev/tools/pub/cmd/pub-token for detailed documentation.";

const TOKEN_USAGE: &str = "Usage: dart pub token [arguments...]
-h, --help    Print this usage information.

Available subcommands:
  add      Add an authentication token for a package repository.
  list     List servers for which a token exists.
  remove   Remove secret token for package repository at <hosted-url>.";

const ADD_USAGE: &str = "Usage: dart pub token add [options] <hosted-url>
-h, --help                  Print this usage information.
    --env-var=<VARIABLE>    Read the secret token from this environment variable
                            when making requests.";

const LIST_USAGE: &str = "Usage: dart pub token list <subcommand> [arguments...]
-h, --help    Print this usage information.";

const REMOVE_USAGE: &str = "Usage: dart pub token remove <hosted-url> | --all
-h, --help    Print this usage information.
    --all     Remove all secret tokens.";

const LOGOUT_USAGE: &str = "Usage: dart pub logout <subcommand> [arguments...]
-h, --help    Print this usage information.";

/// `applicationConfigHome('dart')` of package:cli_util.
pub(crate) fn config_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support")
    } else {
        match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            Some(xdg) => PathBuf::from(xdg),
            None => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
        }
    };
    Some(base.join("dart"))
}

fn tokens_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("pub-tokens.json"))
}

/// package:args' `wrapText` at 80 columns.
pub(crate) fn wrap_text(text: &str, length: usize) -> String {
    let mut result = Vec::new();
    for line in text.split('\n') {
        let trimmed = line.trim_start();
        let leading = &line[..line.len() - trimmed.len()];
        for wrapped in wrap_lines(trimmed, length.saturating_sub(leading.len())) {
            result.push(if wrapped.is_empty() {
                String::new()
            } else {
                format!("{leading}{wrapped}")
            });
        }
    }
    result.join("\n")
}

/// package:args' `wrapTextAsLines`.
fn wrap_lines(text: &str, length: usize) -> Vec<String> {
    let effective = length.max(10);
    let mut result = Vec::new();
    for line in text.split('\n') {
        let chars: Vec<char> = line.trim().chars().collect();
        if chars.len() <= effective {
            result.push(chars.iter().collect());
            continue;
        }
        let white = |i: usize| chars.get(i).is_some_and(|c| c.is_whitespace());
        let mut start = 0;
        let mut last_white: Option<usize> = None;
        let mut i = 0;
        while i < chars.len() {
            if white(i) {
                last_white = Some(i);
            }
            if i - start >= effective {
                if let Some(w) = last_white {
                    i = w;
                }
                result.push(chars[start..i].iter().collect::<String>().trim().to_owned());
                while i < chars.len() && white(i) {
                    i += 1;
                }
                start = i;
                last_white = None;
            }
            i += 1;
        }
        result.push(chars[start..].iter().collect::<String>().trim().to_owned());
    }
    result
}

fn usage_error(message: &str, usage_text: &str, docs: bool) -> Failure {
    let mut text = format!("{message}\n\n{usage_text}\n\n{FOOTER}");
    if docs {
        text.push('\n');
        text.push_str(DOCS);
    }
    usage(text)
}

fn print_help(description: &str, usage_text: &str, docs: bool) {
    let mut text = format!("{}\n\n{usage_text}\n\n{FOOTER}", wrap_text(description, 80));
    if docs {
        text.push('\n');
        text.push_str(DOCS);
    }
    println!("{text}");
}

fn parse(args: &[String], specs: &[Spec], command: &str) -> Result<Arguments, Failure> {
    let mut all: Vec<Spec> = Vec::new();
    for spec in specs.iter().chain(COMMON.iter()) {
        all.push(Spec {
            name: spec.name,
            abbr: spec.abbr,
            value: spec.value,
            negatable: spec.negatable,
        });
    }
    Arguments::parse(args, &all, command)
}

fn tokens_path_text() -> String {
    tokens_file().map_or_else(|| "null".to_owned(), |p| p.display().to_string())
}

/// `validateAndNormalizeHostedUrl`, with `FormatException.message` only.
fn validate_url(text: &str) -> Result<String, String> {
    match url::Url::parse(text) {
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            Err("url scheme must be https:// or http://".into())
        }
        _ => super::hosted::normalize_host(text).map_err(|e| match e.rfind(" (") {
            Some(index) if e.ends_with(')') && !e.starts_with("invalid url") => {
                e[..index].to_owned()
            }
            _ => e,
        }),
    }
}

/// One entry of `pub-tokens.json`, in its written key order.
pub(crate) struct Credential {
    pub url: String,
    pub fields: Vec<(String, Node)>,
}

impl Credential {
    fn to_json(&self) -> String {
        let mut entries = vec![("url".to_owned(), Node::String(self.url.clone()))];
        entries.extend(
            ["token", "env"]
                .iter()
                .filter_map(|k| self.fields.iter().find(|(n, _)| n == k).cloned()),
        );
        entries.extend(
            self.fields
                .iter()
                .filter(|(n, _)| n != "token" && n != "env" && n != "url")
                .cloned(),
        );
        Node::Map(entries).to_json()
    }
}

/// `TokenStore._loadCredentials`.
pub(crate) fn load_credentials() -> Vec<Credential> {
    let mut result = Vec::new();
    let Some(path) = tokens_file() else {
        return result;
    };
    let Ok(text) = std::fs::read(&path) else {
        return result;
    };
    let fail = |message: &str| eprintln!("Failed to load pub-tokens.json: {message}");
    let Ok(json) = parse_json(&text) else {
        fail(&format!("{} is not valid JSON", path.display()));
        return result;
    };
    if !matches!(json, Node::Map(_)) {
        fail("JSON contents is corrupted or not supported");
        return result;
    }
    if json.get("version").to_json() != "1" {
        fail("Version is not supported");
        return result;
    }
    if !json.contains("hosted") {
        return result;
    }
    let Some(hosted) = json.get("hosted").as_list() else {
        fail("Invalid or not supported format");
        return result;
    };
    for element in hosted {
        let url = element.get("url").as_str().map(str::to_owned);
        let problem = (|| -> Result<Credential, String> {
            let Node::Map(entries) = element else {
                return Err("Invalid or not supported format".into());
            };
            let url = element
                .get("url")
                .as_str()
                .ok_or("Url is not provided for the credential")?;
            let url = validate_url(url)?;
            for key in ["token", "env"] {
                if element.contains(key) && !matches!(element.get(key), Node::String(_)) {
                    return Err(format!("Provided \"{key}\" value should be a string"));
                }
            }
            let credential = Credential {
                url,
                fields: entries
                    .iter()
                    .filter(|(k, _)| k != "url")
                    .cloned()
                    .collect(),
            };
            Ok(credential)
        })();
        // Like Pub, a credential with neither or both of `token` and `env` is
        // kept in the list (it is added before `isValid` is checked).
        let problem = problem.and_then(|credential| {
            let token = credential.fields.iter().any(|(k, _)| k == "token");
            let env = credential.fields.iter().any(|(k, _)| k == "env");
            result.push(credential);
            if token == env {
                Err("Invalid or not supported credential".to_owned())
            } else {
                Ok(())
            }
        });
        match problem {
            Ok(()) => {}
            Err(message) => match url {
                Some(url) => eprintln!("Failed to load credentials for {url}: {message}"),
                None => {
                    eprintln!("Failed to load credentials for unknown hosted repository: {message}")
                }
            },
        }
    }
    result
}

pub(crate) fn save_credentials(credentials: &[Credential]) -> Result<(), Failure> {
    let path = tokens_file().ok_or_else(missing_config_dir)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let hosted: Vec<String> = credentials.iter().map(Credential::to_json).collect();
    std::fs::write(
        &path,
        format!("{{\"version\":1,\"hosted\":[{}]}}", hosted.join(",")),
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

/// The `Authorization` header Pub's authenticated client adds to a request
/// for `url` made for the package server `server`: the bearer token of the
/// server's first valid credential, when the URL is below it.
pub(crate) fn authorization_for(server: &str, url: &str) -> Option<String> {
    /// Each server with its token (`None` when its variable is unset).
    type Tokens = Vec<(String, Option<String>)>;
    static CREDENTIALS: std::sync::Mutex<Option<Tokens>> = std::sync::Mutex::new(None);
    let mut guard = CREDENTIALS.lock().ok()?;
    let credentials = guard.get_or_insert_with(|| {
        load_credentials()
            .into_iter()
            .filter_map(|c| {
                let field = |k: &str| {
                    c.fields
                        .iter()
                        .find(|(n, _)| n == k)
                        .map(|(_, v)| v.as_str().map(str::to_owned))
                };
                match (field("token"), field("env")) {
                    (Some(token), None) => Some((c.url, token)),
                    (None, Some(Some(env))) => Some((c.url, std::env::var(env).ok())),
                    _ => None,
                }
            })
            .collect()
    });
    let normalize = |u: &str| {
        let u = if u.ends_with('/') {
            u.to_owned()
        } else {
            format!("{u}/")
        };
        u.to_lowercase()
    };
    let (credential_url, token) = credentials.iter().find(|(u, _)| u == server)?;
    let token = token.as_ref()?;
    (normalize(url).starts_with(&normalize(credential_url)) && valid_bearer_token(token))
        .then(|| format!("Bearer {token}"))
}

fn missing_config_dir() -> Failure {
    let variable = if cfg!(windows) { "%APPDATA%" } else { "$HOME" };
    data(format!("No config dir found. Check that {variable} is set"))
}

/// Runs `dart pub token <args>`.
pub(crate) fn run(args: &[String]) -> Result<u8, Failure> {
    let Some(first) = args.first() else {
        return Err(usage_error(
            "Missing subcommand for \"dart pub token\".",
            TOKEN_USAGE,
            true,
        ));
    };
    let rest = &args[1..];
    match first.as_str() {
        "-h" | "--help" => {
            print_help(
                &format!(
                    "Manage authentication tokens for hosted pub repositories.\n\nThe tokens will be used for authorizing both when retrieving dependencies and\nfor publishing.\n\nTokens are stored in `{}`.\n\nFor interactive authorization against pub.dev, use `dart pub login`.",
                    tokens_path_text()
                ),
                TOKEN_USAGE,
                true,
            );
            Ok(0)
        }
        "add" => add(rest),
        "list" => list(rest),
        "remove" => remove(rest),
        other if other.starts_with('-') => Err(usage_error(
            &format!(
                "Could not find an option named \"{}\".",
                other.trim_start_matches('-')
            ),
            TOKEN_USAGE,
            true,
        )),
        other => Err(usage_error(
            &format!("Could not find a subcommand named \"{other}\" for \"dart pub token\"."),
            TOKEN_USAGE,
            true,
        )),
    }
}

fn add(args: &[String]) -> Result<u8, Failure> {
    let arguments = parse(args, &[option("env-var", None)], "token add")?;
    if arguments.is("help") {
        print_help(
            &format!(
                "Add an authentication token for a package repository.\n\nThe token will be used for authorizing against <hosted-url> both when\nretrieving dependencies and for publishing.\n\nTokens are stored in {}.\n\nThis command will prompt for the secret token over stdin.\n(Unless `--env-var` is used).\n\nFor interactive authorization against pub.dev, use `dart pub login`.",
                tokens_path_text()
            ),
            ADD_USAGE,
            true,
        );
        return Ok(0);
    }
    let raw = match arguments.rest.as_slice() {
        [] => {
            return Err(usage_error(
                "The [hosted-url] for a package repository must be given.",
                ADD_USAGE,
                true,
            ));
        }
        [one] => one.clone(),
        _ => {
            return Err(usage_error(
                "Takes only a single argument.",
                ADD_USAGE,
                true,
            ));
        }
    };
    let invalid = |message: &str| {
        usage_error(
            &format!("Invalid [hosted-url]: \"{raw}\"\n{message}"),
            ADD_USAGE,
            true,
        )
    };
    let url = validate_url(&raw).map_err(|e| invalid(&e))?;
    let host = url::Url::parse(&url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default();
    let localhost = ["localhost", "127.0.0.1", "[::1]"].contains(&host.as_str());
    if !url.starts_with("https:") && !localhost {
        return Err(invalid(
            "url must be https://, insecure repositories cannot use authentication.",
        ));
    }
    let credential = match arguments.option("env-var") {
        None => {
            print!("Enter secret token: ");
            let _ = std::io::stdout().flush();
            let mut token = String::new();
            let _ = std::io::stdin().lock().read_line(&mut token);
            let token = token.trim_end_matches(['\n', '\r']).trim().to_owned();
            if token.is_empty() {
                return Err(usage_error("Token is not provided.", ADD_USAGE, true));
            }
            if !valid_bearer_token(&token) {
                return Err(data(
                    "The entered token is not a valid Bearer token. A token may only contain `a-zA-Z0-9._~+/=-`",
                ));
            }
            Credential {
                url: url.clone(),
                fields: vec![("token".into(), Node::String(token))],
            }
        }
        Some(variable) => {
            if variable.is_empty() {
                return Err(usage_error(
                    "Cannot use the empty string as --env-var",
                    ADD_USAGE,
                    true,
                ));
            }
            if variable.contains('=') {
                return Err(data(format!(
                    "Environment variable name --env-var=\"{variable}\" cannot contain \"=\", the equals sign is not allowed in environment variable names."
                )));
            }
            let conventional = variable
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase() || c == '_')
                && variable
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
            if !conventional {
                eprintln!(
                    "The environment variable name --env-var=\"{variable}\" does not use uppercase characters A-Z, 0-9 and underscore. This is unusual for environment variable names.\nCheck that you meant to use the environment variable name: \"{variable}\"."
                );
            }
            Credential {
                url: url.clone(),
                fields: vec![("env".into(), Node::String(variable.to_owned()))],
            }
        }
    };
    tokens_file().ok_or_else(missing_config_dir)?;
    let mut credentials = load_credentials();
    credentials.retain(|c| c.url != url);
    let env = credential
        .fields
        .iter()
        .find(|(k, _)| k == "env")
        .and_then(|(_, v)| v.as_str().map(str::to_owned));
    credentials.push(credential);
    save_credentials(&credentials)?;
    match env {
        None => println!("Requests to \"{url}\" will now be authenticated using the secret token."),
        Some(variable) => {
            println!(
                "Requests to \"{url}\" will now be authenticated using the secret token stored in the environment variable \"{variable}\"."
            );
            if std::env::var_os(&variable).is_none() {
                eprintln!("Environment variable \"{variable}\" is not defined.");
            }
        }
    }
    Ok(0)
}

/// `Credential.isValidBearerToken`.
pub(crate) fn valid_bearer_token(token: &str) -> bool {
    !token.is_empty()
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._~+/=-".contains(&b))
}

fn list(args: &[String]) -> Result<u8, Failure> {
    let arguments = parse(args, &[], "token list")?;
    if arguments.is("help") {
        print_help("List servers for which a token exists.", LIST_USAGE, true);
        return Ok(0);
    }
    if !arguments.rest.is_empty() {
        return Err(usage_error(
            "Command \"list\" does not take any arguments.",
            LIST_USAGE,
            true,
        ));
    }
    let credentials = load_credentials();
    if credentials.is_empty() {
        println!(
            "You do not have any secret tokens for package repositories.\nHowever you can add new tokens using the command below:\n\n    pub token add [hosted-url]"
        );
    } else {
        println!(
            "You have secret tokens for {} package repositories:",
            credentials.len()
        );
        for credential in &credentials {
            println!("{}", credential.url);
        }
    }
    Ok(0)
}

fn remove(args: &[String]) -> Result<u8, Failure> {
    let arguments = parse(args, &[flag("all", None, false)], "token remove")?;
    if arguments.is("help") {
        print_help(
            "Remove secret token for package repository at <hosted-url>.",
            REMOVE_USAGE,
            true,
        );
        return Ok(0);
    }
    if arguments.is("all") {
        let count = load_credentials().len();
        let path = tokens_file().ok_or_else(missing_config_dir)?;
        if path.is_file() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            println!("pub-tokens.json is deleted.");
        } else {
            println!("No credentials file found at \"{}\"", path.display());
        }
        println!("Removed {count} secret tokens.");
        return Ok(0);
    }
    let raw = match arguments.rest.as_slice() {
        [] => {
            return Err(usage_error(
                "The [hosted-url] for a package repository must be specified.",
                REMOVE_USAGE,
                true,
            ));
        }
        [one] => one.clone(),
        _ => {
            return Err(usage_error(
                "Takes only a single argument.",
                REMOVE_USAGE,
                true,
            ));
        }
    };
    let url = validate_url(&raw).map_err(|e| {
        usage_error(
            &format!("Invalid [hosted-url]: \"{raw}\"\n{e}"),
            REMOVE_USAGE,
            true,
        )
    })?;
    tokens_file().ok_or_else(missing_config_dir)?;
    let mut credentials = load_credentials();
    let before = credentials.len();
    credentials.retain(|c| c.url != url);
    if credentials.len() == before {
        return Err(data(format!(
            "No secret token for package repository \"{url}\" was found."
        )));
    }
    save_credentials(&credentials)?;
    println!("Removed secret token for package repository: {url}");
    Ok(0)
}

/// `dart pub logout`: deletes the OAuth2 credentials of pub.dev.
pub(crate) fn logout(args: &[String]) -> Result<u8, Failure> {
    let arguments = parse(args, &[], "logout")?;
    if arguments.is("help") {
        println!("Log out of pub.dev.\n\n{LOGOUT_USAGE}\n\n{FOOTER}");
        return Ok(0);
    }
    if !arguments.rest.is_empty() {
        return Err(usage_error(
            "Command \"logout\" does not take any arguments.",
            LOGOUT_USAGE,
            false,
        ));
    }
    let file = config_dir().map(|d| d.join("pub-credentials.json"));
    match &file {
        Some(path) if path.exists() => {
            println!("Logging out of pub.dev.");
            println!("Deleting {}", path.display());
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        _ => println!(
            "No existing credentials file {}. Cannot log out.",
            file.map_or_else(|| "null".to_owned(), |p| p.display().to_string())
        ),
    }
    Ok(0)
}

/// `dart pub uploader`: Pub 3.6.2 only points to pub.dev's admin page.
pub(crate) fn uploader(args: &[String], settings_dir: &str) -> Result<u8, Failure> {
    let specs = [
        option("server", None),
        option("package", None),
        option("directory", Some('C')),
    ];
    let arguments = parse(args, &specs, "uploader")?;
    let directory = arguments.option("directory").unwrap_or(settings_dir);
    let name = std::fs::read_to_string(std::path::Path::new(directory).join("pubspec.yaml"))
        .ok()
        .and_then(|t| super::tree::parse_yaml(&t).ok())
        .and_then(|n| n.get("name").as_str().map(str::to_owned))
        .unwrap_or_else(|| "<packageName>".to_owned());
    Err(format!(
        "Package uploaders are no longer managed from the command line.\nManage uploaders from:\n\nhttps://pub.dev/packages/{name}/admin"
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_text_wraps_like_package_args() {
        let path = format!(
            "/{}/Library/Application Support/dart/pub-tokens.json",
            "x".repeat(90)
        );
        let text = wrap_text(&format!("Tokens are stored in `{path}`."), 80);
        let lines: Vec<&str> = text.split('\n').collect();
        assert_eq!(lines[0], "Tokens are stored in");
        assert_eq!(
            lines[1].chars().count(),
            80,
            "long words are cut at the width"
        );
        assert!(lines.iter().all(|l| l.chars().count() <= 80));
        assert_eq!(wrap_text("short\n\n  indented", 80), "short\n\n  indented");
    }

    #[test]
    fn hosted_urls_are_validated_with_pub_messages() {
        assert_eq!(
            validate_url("https://example.com/").unwrap(),
            "https://example.com"
        );
        assert_eq!(
            validate_url("https://pub.dartlang.org").unwrap(),
            "https://pub.dev"
        );
        assert_eq!(
            validate_url("not-a-url").unwrap_err(),
            "url scheme must be https:// or http://"
        );
        assert_eq!(
            validate_url("https://a.b/?q=1").unwrap_err(),
            "querystring is not supported in url"
        );
        assert!(valid_bearer_token("abc.DEF_123~+/=-"));
        assert!(!valid_bearer_token("bad%token"));
    }

    #[test]
    fn credentials_keep_pub_key_order() {
        let credential = Credential {
            url: "https://example.com".into(),
            fields: vec![
                ("extra".into(), Node::Bool(true)),
                ("env".into(), Node::String("TOKEN".into())),
            ],
        };
        assert_eq!(
            credential.to_json(),
            r#"{"url":"https://example.com","env":"TOKEN","extra":true}"#
        );
    }
}
