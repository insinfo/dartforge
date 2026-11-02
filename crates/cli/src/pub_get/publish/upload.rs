//! The upload of `lish.dart`: the confirmation, the three requests of the
//! publishing protocol (`GET api/packages/versions/new`, the multipart
//! `POST` to the URL it returns, `GET` of the `location` it redirects to),
//! bearer tokens from `pub-tokens.json`, Pub's retries and its error
//! reports. Reading the pubspec of an archive for `--from-archive` is here
//! too.
use super::{Options, Publication, terminal};
use crate::pub_get::{
    Failure, Settings, data, exit,
    token::{Credential, load_credentials, save_credentials, valid_bearer_token},
    tree::{Node, parse_json},
};
use std::io::{BufRead, Read, Write};

/// `extractFileFromTarGz(bytes, 'pubspec.yaml')` decoded as UTF-8; the
/// error is the `FormatException`'s message.
pub(super) fn pubspec_from_archive(bytes: &[u8]) -> Result<String, String> {
    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut tar)
        .map_err(|e| e.to_string())?;
    let mut archive = tar::Archive::new(tar.as_slice());
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .path()
            .map(|p| super::paths::normalize(&p.to_string_lossy()))
            .unwrap_or_default();
        if name != "pubspec.yaml" {
            continue;
        }
        if !matches!(
            entry.header().entry_type(),
            tar::EntryType::Regular | tar::EntryType::Continuous
        ) {
            return Err("pubspec.yaml is not a file".into());
        }
        let mut contents = Vec::new();
        entry
            .read_to_end(&mut contents)
            .map_err(|e| e.to_string())?;
        return String::from_utf8(contents).map_err(|_| "Unexpected extension byte".into());
    }
    Err("Could not find pubspec.yaml in archive".into())
}

/// `_confirmUpload`.
pub(super) fn confirm(
    options: &Options,
    publication: &Publication,
    host: &str,
) -> Result<(), Failure> {
    if options.force {
        return Ok(());
    }
    terminal::message(
        "\nPublishing is forever; packages cannot be unpublished.\nPolicy details are available at https://pub.dev/policy\n",
    );
    let mut message = format!(
        "Do you want to publish {} {} to {host}",
        publication.name, publication.version
    );
    if publication.hints != 0 || publication.warnings != 0 {
        message = format!("{}. {message}", publication.warnings_count_message());
    }
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "\n{message} (y/N)? ");
    let _ = out.flush();
    let mut reply = String::new();
    let _ = std::io::stdin().lock().read_line(&mut reply);
    if !reply.starts_with(['y', 'Y']) {
        return Err(data("Package upload canceled."));
    }
    Ok(())
}

/// A response: status, reason, headers we read and the body.
struct Response {
    url: String,
    status: u16,
    reason: String,
    location: Option<String>,
    authenticate: Option<String>,
    body: String,
}

/// Why a request failed, as the Dart exceptions Pub distinguishes.
enum HttpError {
    /// `PubHttpResponseException` for a status outside 2xx/3xx.
    Status(Box<Response>),
    /// `AuthenticationException` (401 or 403) with the server's message.
    Authentication(u16, Option<String>),
    /// A connection error (`http.ClientException`/`SocketException`).
    Io(String),
    /// A failure Pub reports directly (`fail`/`dataError`).
    Fail(Failure),
}

/// The credential of `host` (`TokenStore.findCredential`).
fn find_credential(credentials: &[Credential], host: &str) -> Option<usize> {
    let valid = |c: &Credential| {
        let token = c.fields.iter().any(|(k, _)| k == "token");
        let env = c.fields.iter().any(|(k, _)| k == "env");
        token != env
    };
    let mut found = None;
    for (i, credential) in credentials.iter().enumerate() {
        if credential.url == host && valid(credential) {
            if found.is_none() {
                found = Some(i);
            } else {
                terminal::warning(&format!(
                    "Found multiple matching authentication tokens for \"{host}\". First matching token will be used for authentication."
                ));
                break;
            }
        }
    }
    found
}

fn normalize_for_match(url: &str) -> String {
    let url = if url.ends_with('/') {
        url.to_owned()
    } else {
        format!("{url}/")
    };
    url.to_lowercase()
}

/// The `Authorization` value of a credential (`getAuthorizationHeaderValue`).
fn authorization(credential: &Credential) -> Result<String, Failure> {
    let field = |key: &str| {
        credential
            .fields
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.as_str().map(str::to_owned))
    };
    let token = match field("env") {
        Some(env) => std::env::var(&env).map_err(|_| {
            data(format!(
                "Saved credential for \"{}\" pub repository requires environment variable named \"{env}\" but not defined.",
                credential.url
            ))
        })?,
        None => field("token").unwrap_or_default(),
    };
    if !valid_bearer_token(&token) {
        return Err(data(format!(
            "Credential token for {} is not a valid Bearer token. It should match `^[a-zA-Z0-9._~+/=-]+$`",
            credential.url
        )));
    }
    Ok(format!("Bearer {token}"))
}

/// `sanitizeForTerminal`.
fn sanitize(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if (' '..='\u{7f}').contains(&c) {
                c
            } else {
                ' '
            }
        })
        .take(1024)
        .collect()
}

/// The `message` parameter of a `Bearer realm="pub"` challenge.
fn server_message(header: &str) -> Option<String> {
    let rest = header.trim_start();
    let (scheme, params) = rest.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let mut realm = None;
    let mut message = None;
    let mut chars = params.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| *c == ' ' || *c == ',') {
            chars.next();
        }
        let key: String = std::iter::from_fn(|| chars.next_if(|c| *c != '=')).collect();
        if key.is_empty() || chars.next() != Some('=') {
            break;
        }
        let value: String = if chars.peek() == Some(&'"') {
            chars.next();
            let mut v = String::new();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        if let Some(n) = chars.next() {
                            v.push(n);
                        }
                    }
                    '"' => break,
                    c => v.push(c),
                }
            }
            v
        } else {
            std::iter::from_fn(|| chars.next_if(|c| *c != ',' && *c != ' ')).collect()
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "realm" => realm = Some(value),
            "message" => message = Some(value),
            _ => {}
        }
    }
    (realm.as_deref() == Some("pub"))
        .then_some(message)
        .flatten()
}

struct Client {
    agent: ureq::Agent,
    host: String,
    authorization: Option<String>,
    /// A 401 was seen: the credential is deleted afterwards.
    invalid: bool,
}

impl Client {
    fn send(
        &mut self,
        method: &str,
        url: &str,
        api: bool,
        body: Option<(&str, Vec<u8>)>,
    ) -> Result<Response, HttpError> {
        let mut builder = ureq::http::Request::builder()
            .method(method)
            .uri(url)
            .header("user-agent", "Dart pub 3.6.2");
        if api {
            builder = builder.header("Accept", "application/vnd.pub.v2+json");
        }
        if let Some(value) = &self.authorization
            && normalize_for_match(url).starts_with(&normalize_for_match(&self.host))
        {
            builder = builder.header("authorization", value.as_str());
        }
        let response = match body {
            Some((content_type, bytes)) => {
                let request = builder
                    .header("content-type", content_type)
                    .body(bytes)
                    .map_err(|e| HttpError::Io(e.to_string()))?;
                self.agent.run(request)
            }
            None => {
                let request = builder.body(()).map_err(|e| HttpError::Io(e.to_string()))?;
                self.agent.run(request)
            }
        };
        let mut response = response.map_err(|e| HttpError::Io(e.to_string()))?;
        let status = response.status();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let location = header("location");
        let authenticate = header("www-authenticate");
        let body = response.body_mut().read_to_string().unwrap_or_default();
        let result = Response {
            url: url.to_owned(),
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("").to_owned(),
            location,
            authenticate,
            body,
        };
        if result.status == 401 {
            self.invalid = true;
        }
        if result.status == 401 || result.status == 403 {
            let message = result
                .authenticate
                .as_deref()
                .and_then(server_message)
                .map(|m| sanitize(&m));
            return Err(HttpError::Authentication(result.status, message));
        }
        Ok(result)
    }

    /// `client.fetch` with `throwIfNotOk`, inside `retryForHttp`.
    fn fetch(
        &mut self,
        method: &str,
        url: &str,
        api: bool,
        body: impl Fn() -> Option<(String, Vec<u8>)>,
    ) -> Result<Response, HttpError> {
        let attempts = std::env::var("PUB_MAX_HTTP_RETRIES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(7)
            .max(1);
        let mut attempt = 0;
        loop {
            attempt += 1;
            let made = body();
            let result = self.send(
                method,
                url,
                api,
                made.as_ref().map(|(t, b)| (t.as_str(), b.clone())),
            );
            let retry = match &result {
                Ok(r) => {
                    if (200..=299).contains(&r.status)
                        || [301, 302, 303, 307, 308].contains(&r.status)
                    {
                        return result;
                    }
                    if r.status == 406 && api {
                        return Err(HttpError::Fail(Failure::from(format!(
                            "Pub 3.6.2 is incompatible with the current version of {}.\nUpgrade pub to the latest version and try again.",
                            url::Url::parse(url)
                                .ok()
                                .and_then(|u| u.host_str().map(str::to_owned))
                                .unwrap_or_default()
                        ))));
                    }
                    r.status >= 500 || r.status == 408 || r.status == 429
                }
                Err(HttpError::Io(_)) => true,
                Err(_) => false,
            };
            if !retry || attempt >= attempts {
                return match result {
                    Ok(r) => Err(HttpError::Status(Box::new(r))),
                    Err(e) => Err(e),
                };
            }
            // `package:retry`: 200 ms doubled per attempt, at most 30 s.
            let delay = (200u64 << (attempt - 1).min(16)).min(30_000);
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
    }
}

/// `invalidServerResponse`.
fn invalid_response(response: &Response) -> HttpError {
    HttpError::Fail(Failure::from(terminal::red(&format!(
        "Invalid server response:\n{}",
        response.body
    ))))
}

fn parse_map(response: &Response) -> Result<Node, HttpError> {
    match parse_json(response.body.as_bytes()) {
        Ok(node @ Node::Map(_)) => Ok(node),
        _ => Err(invalid_response(response)),
    }
}

/// `handleJsonError`.
fn json_error(response: &Response) -> Failure {
    let map = match parse_map(response) {
        Ok(map) => map,
        Err(HttpError::Fail(f)) => return f,
        Err(_) => unreachable!("parse_map fails only with Fail"),
    };
    match map.get("error").get("message") {
        Node::String(message) => Failure::from(format!(
            "Message from server: {}",
            terminal::red(&sanitize(message))
        )),
        _ => match invalid_response(response) {
            HttpError::Fail(f) => f,
            _ => unreachable!(),
        },
    }
}

/// The origin of a URL (`scheme://host:port`), as Pub compares them; the
/// request URL of `localhost` is rewritten to `127.0.0.1` before sending.
fn origin(url: &str, requested: bool) -> String {
    match url::Url::parse(url) {
        Ok(u) => {
            let host = u.host_str().unwrap_or("");
            let host = if requested && host == "localhost" {
                "127.0.0.1"
            } else {
                host
            };
            match u.port() {
                Some(port) => format!("{}://{host}:{port}", u.scheme()),
                None => format!("{}://{host}", u.scheme()),
            }
        }
        Err(_) => url.to_owned(),
    }
}

/// The URL as Pub's `_PubHttpClient` sends it.
fn requested(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) if u.host_str() == Some("localhost") => {
            let _ = u.set_host(Some("127.0.0.1"));
            u.to_string()
        }
        _ => url.to_owned(),
    }
}

/// The multipart body of `http.MultipartRequest` with `fields` and the
/// archive as `file`.
fn multipart(fields: &[(String, String)], archive: &[u8]) -> (String, Vec<u8>) {
    let boundary = format!(
        "dart-http-boundary-{}",
        crate::pub_get::output::iso8601(true)
            .bytes()
            .filter(u8::is_ascii_digit)
            .map(char::from)
            .collect::<String>()
    );
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\ncontent-disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\ncontent-type: application/octet-stream\r\ncontent-disposition: form-data; name=\"file\"; filename=\"package.tar.gz\"\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(archive);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

/// The crash report of `PubCommand.run` for an exception Pub does not
/// expect (`PubHttpResponseException`), with exit code 69.
fn unexpected(settings: &Settings, message: &str) -> Failure {
    let log = settings.cache_root.join("log").join("pub_log.txt");
    let _ = std::fs::create_dir_all(settings.cache_root.join("log"));
    let _ = std::fs::write(
        &log,
        format!(
            "Information about the latest pub run.\n\nIf you believe something is not working right, you can go to\nhttps://github.com/dart-lang/pub/issues/new to post a new issue and attach this file.\n\nBefore making this file public, make sure to remove any sensitive information!\n\nPub version: 3.6.2\nCreated: {}\nFLUTTER_ROOT: {}\nPUB_HOSTED_URL: {}\nPUB_CACHE: \"{}\"\nCommand: dart pub publish\nPlatform: {}\n\n---- Log transcript ----\nERR : {message}\n---- End log transcript ----\n",
            crate::pub_get::output::iso8601(false),
            std::env::var("FLUTTER_ROOT").unwrap_or_else(|_| "<not set>".into()),
            std::env::var("PUB_HOSTED_URL").unwrap_or_else(|_| "<not set>".into()),
            std::env::var("PUB_CACHE").unwrap_or_else(|_| "<not set>".into()),
            std::env::consts::OS
        ),
    );
    Failure {
        code: exit::UNAVAILABLE,
        message: format!(
            "{message}\nThis is an unexpected error. The full log and other details are collected in:\n\n    {}\n\nConsider creating an issue on https://github.com/dart-lang/pub/issues/new\nand attaching the relevant parts of that log file.",
            log.display()
        ),
    }
}

/// `_publish`: uploads `archive` to `host`.
pub(super) fn publish(settings: &Settings, archive: &[u8], host: &str) -> Result<(), Failure> {
    let credentials = load_credentials();
    let credential = find_credential(&credentials, host);
    if host == "https://pub.dev" && credential.is_none() {
        return Err(Failure::from(
            "Publishing to https://pub.dev needs the OAuth2 login of `dart pub login`, which dartforge does not implement.\nAdd a token for the server with `dart pub token add https://pub.dev` instead.",
        ));
    }
    let authorization = credential
        .map(|i| authorization(&credentials[i]))
        .transpose()?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(600)))
        .build()
        .into();
    let mut client = Client {
        agent,
        host: host.to_owned(),
        authorization,
        invalid: false,
    };
    let result = upload(&mut client, settings, archive, host);
    if client.invalid
        && let Some(i) = credential
    {
        let mut credentials = credentials;
        let url = credentials[i].url.clone();
        credentials.retain(|c| c.url != url);
        save_credentials(&credentials)?;
        terminal::warning(&format!("Invalid token for {host} deleted."));
    }
    result
}

fn upload(
    client: &mut Client,
    settings: &Settings,
    archive: &[u8],
    host: &str,
) -> Result<(), Failure> {
    terminal::message("Uploading...");
    let new = crate::pub_get::hosted::api_url(host, "api/packages/versions/new");
    let mut cloud_storage: Option<String> = None;
    let outcome = (|| -> Result<(), HttpError> {
        let response = client.fetch("GET", &requested(&new), true, || None)?;
        let parameters = parse_map(&response)?;
        let Node::String(url) = parameters.get("url") else {
            return Err(invalid_response(&response));
        };
        cloud_storage = Some(url.clone());
        let fields = match parameters.get("fields") {
            Node::Map(entries) => {
                let mut fields = Vec::new();
                for (key, value) in entries {
                    let Node::String(value) = value else {
                        return Err(invalid_response(&response));
                    };
                    fields.push((key.clone(), value.clone()));
                }
                fields
            }
            _ => return Err(invalid_response(&response)),
        };
        let upload = client.fetch("POST", &requested(url), false, || {
            Some(multipart(&fields, archive))
        })?;
        let Some(location) = upload.location.clone() else {
            return Err(HttpError::Status(Box::new(upload)));
        };
        let finalize = client.fetch("GET", &requested(&location), true, || None)?;
        match parse_map(&finalize)?.get("success").get("message") {
            Node::String(message) => {
                terminal::message(&format!(
                    "Message from server: {}",
                    green(&sanitize(message))
                ));
                Ok(())
            }
            _ => Err(invalid_response(&finalize)),
        }
    })();
    match outcome {
        Ok(()) => Ok(()),
        Err(HttpError::Fail(f)) => Err(f),
        Err(HttpError::Authentication(status, message)) => {
            let mut text = String::new();
            if status == 401 {
                text.push_str(&format!(
                    "{host} package repository requested authentication!\nYou can provide credentials using:\n    dart pub token add {host}\n"
                ));
            }
            if status == 403 {
                text.push_str(&format!(
                    "Insufficient permissions to the resource at the {host} package repository.\nYou can modify credentials using:\n    dart pub token add {host}\n"
                ));
            }
            if let Some(message) = message {
                text.push_str(&format!("\n{message}\n"));
            }
            Err(data(format!(
                "{text}{}",
                terminal::red("Authentication failed!")
            )))
        }
        Err(HttpError::Status(response)) => {
            let same_url = cloud_storage
                .as_deref()
                .is_some_and(|u| requested(u) == u && response.url == u);
            if same_url {
                gcs_error(&response.body);
                return Err(Failure::from(terminal::red(
                    "Failed to upload the package.",
                )));
            }
            if origin(&response.url, false) == origin(host, false) {
                return Err(json_error(&response));
            }
            Err(unexpected(
                settings,
                &format!("HTTP error {} {}", response.status, response.reason),
            ))
        }
        Err(HttpError::Io(message)) => Err(Failure {
            code: exit::UNAVAILABLE,
            message,
        }),
    }
}

fn green(text: &str) -> String {
    if terminal::colors() {
        format!("\u{1b}[32m{text}\u{1b}[39m")
    } else {
        text.to_owned()
    }
}

/// `handleGCSError`: the code, message and details of an XML error.
fn gcs_error(body: &str) {
    if !body.contains("<?xml") {
        return;
    }
    let tag = |name: &str| {
        let open = format!("<{name}>");
        let close = format!("</{name}>");
        let start = body.find(&open)? + open.len();
        // `(.*)` is greedy and stops at a line break.
        let line_end = body[start..].find('\n').map_or(body.len(), |i| start + i);
        let end = body[start..line_end].rfind(&close)? + start;
        Some(sanitize(&body[start..end]))
    };
    if let Some(code) = tag("Code") {
        terminal::error(&format!("Server error code: {}", sanitize(&code)));
    }
    if let Some(message) = tag("Message") {
        terminal::error(&format!("Server message: {}", sanitize(&message)));
    }
    if let Some(details) = tag("Details") {
        terminal::error(&format!("Server details: {}", sanitize(&details)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_server_message_of_a_challenge() {
        assert_eq!(
            server_message("Bearer realm=\"pub\", message=\"Go away\""),
            Some("Go away".into())
        );
        assert_eq!(
            server_message("Bearer realm=\"other\", message=\"x\""),
            None
        );
        assert_eq!(server_message("Basic realm=\"pub\""), None);
    }

    #[test]
    fn sanitizes_like_pub() {
        assert_eq!(sanitize("a\nb\u{e9}"), "a b ");
    }
}
