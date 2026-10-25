//! Inert project metadata, separate from chat work and its replay identities.
use super::*;

#[cfg(test)]
#[path = "app_projects_tests.rs"]
mod tests;

const MAX_PROJECTS: usize = 32;
const MAX_REPOSITORIES: usize = 16;
const MAX_LOCATOR_BYTES: usize = 2048;
const MAX_NAME_SCALARS: usize = 40;
const MAX_KEY_BYTES: usize = 40;
const MAX_DESCRIPTION_SCALARS: usize = 60;
const MAX_PROMPT_SCALARS: usize = 1000;
const MAX_RECEIPTS: usize = 32;
const MAX_TOMBSTONES: usize = 1024;

// Serde structs also accept positional arrays. Project records require
// named fields; do not silently normalize malformed encodings on the next save.
pub(super) fn object<'de, D, T>(d: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let fields = serde_json::Map::<String, Value>::deserialize(d)?;
    serde_json::from_value(Value::Object(fields)).map_err(serde::de::Error::custom)
}
fn objects<'de, D, T>(d: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let records = Vec::<serde_json::Map<String, Value>>::deserialize(d)?;
    records
        .into_iter()
        .map(|fields| {
            serde_json::from_value(Value::Object(fields)).map_err(serde::de::Error::custom)
        })
        .collect()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        // Match Dart's trimming of FEFF as well as Unicode whitespace.
        && name.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}') == name
        && name.chars().count() <= MAX_NAME_SCALARS
        && !name.chars().any(char::is_control)
}
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MAX_KEY_BYTES
        && key.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
fn valid_description(description: &str) -> bool {
    description.chars().count() <= MAX_DESCRIPTION_SCALARS
        && !description
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
}
fn valid_prompt(prompt: &str) -> bool {
    prompt.chars().count() <= MAX_PROMPT_SCALARS
        && !prompt
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
}
fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
fn valid_authority(authority: &str) -> bool {
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    valid_host(host)
        && port.is_none_or(|p| {
            !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|n| n > 0)
        })
}
fn valid_locator(locator: &str) -> bool {
    if locator.len() > MAX_LOCATOR_BYTES || !locator.is_ascii() {
        return false;
    }
    let path = if let Some(rest) = locator
        .strip_prefix("https://")
        .or_else(|| locator.strip_prefix("ssh://git@"))
    {
        let Some((authority, path)) = rest.split_once('/') else {
            return false;
        };
        if !valid_authority(authority) {
            return false;
        }
        path
    } else if let Some(rest) = locator.strip_prefix("git@") {
        let Some((host, path)) = rest.split_once(':') else {
            return false;
        };
        if !valid_host(host) {
            return false;
        }
        path
    } else {
        return false;
    };
    path.split('/').all(|segment| {
        !segment.is_empty()
            && !matches!(segment, "." | "..")
            && segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._~-".contains(&b))
    })
}
fn valid_branch(branch: &str) -> bool {
    !branch.is_empty()
        && branch.len() <= 255
        && branch.is_ascii()
        && branch != "HEAD"
        && !branch.starts_with("refs/")
        && branch.split('/').all(|part| {
            !part.is_empty()
                && part.as_bytes()[0].is_ascii_alphanumeric()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                && !part.contains("..")
                && !part.ends_with('.')
                && !part.to_ascii_lowercase().ends_with(".lock")
        })
}
fn valid_pattern(pattern: &str) -> bool {
    pattern.len() <= 255 && pattern.strip_suffix("/*").is_some_and(valid_branch)
}
fn valid_work(work: &str) -> bool {
    valid_branch(work) || valid_pattern(work)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BranchTypes {
    #[serde(deserialize_with = "required_nullable")]
    feature: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    fix: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    release: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    hotfix: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Branch {
    #[serde(deserialize_with = "required_nullable")]
    base: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    work: Option<String>,
    #[serde(deserialize_with = "object")]
    types: BranchTypes,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Repository {
    #[serde(deserialize_with = "required_nullable")]
    name: Option<String>,
    url: String,
    read_only: bool,
    #[serde(deserialize_with = "object")]
    branch: Branch,
}
impl Repository {
    fn unknown(url: String) -> Self {
        Self {
            name: None,
            url,
            read_only: true,
            branch: Branch {
                base: None,
                work: None,
                types: BranchTypes {
                    feature: None,
                    fix: None,
                    release: None,
                    hotfix: None,
                },
            },
        }
    }
    fn valid(&self) -> bool {
        self.name.as_deref().is_none_or(valid_name)
            && valid_locator(&self.url)
            && self.branch.base.as_deref().is_none_or(valid_branch)
            && self.branch.work.as_deref().is_none_or(valid_work)
            && [
                &self.branch.types.feature,
                &self.branch.types.fix,
                &self.branch.types.release,
                &self.branch.types.hotfix,
            ]
            .iter()
            .all(|p| p.as_deref().is_none_or(valid_pattern))
    }
}
fn valid_metadata(name: &str, repositories: &[Repository], agent: &str) -> bool {
    let mut distinct = HashSet::new();
    let mut names = HashSet::new();
    valid_name(name)
        && agent == "opencode"
        && repositories.len() <= MAX_REPOSITORIES
        && repositories.iter().all(|r| {
            r.valid()
                && distinct.insert(&r.url)
                && r.name.as_ref().is_none_or(|name| names.insert(name))
        })
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    project_id: String,
    name: String,
    key: String,
    description: String,
    prompt: String,
    #[serde(deserialize_with = "objects")]
    repositories: Vec<Repository>,
    agent: String,
}
impl Project {
    fn valid(&self) -> bool {
        valid_metadata(&self.name, &self.repositories, &self.agent)
            && valid_key(&self.key)
            && valid_description(&self.description)
            && valid_prompt(&self.prompt)
    }
}

struct Metadata {
    name: String,
    key: String,
    description: String,
    prompt: String,
    repositories: Vec<Repository>,
    agent: String,
}
impl Metadata {
    fn valid(&self) -> bool {
        valid_metadata(&self.name, &self.repositories, &self.agent)
            && valid_key(&self.key)
            && valid_description(&self.description)
            && valid_prompt(&self.prompt)
            // New writes require IDs; stored and migrated Names remain legacy-compatible.
            && self
                .repositories
                .iter()
                .all(|r| r.name.as_deref().is_some_and(valid_key))
    }
    fn project(self, project_id: String) -> Project {
        Project {
            project_id,
            name: self.name,
            key: self.key,
            description: self.description,
            prompt: self.prompt,
            repositories: self.repositories,
            agent: self.agent,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    request_id: String,
    project_id: String,
    revision: u64,
    deleted: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedReceipt {
    #[serde(deserialize_with = "object")]
    receipt: Receipt,
    #[serde(deserialize_with = "required_nullable")]
    fingerprint: Option<String>,
}
// Frozen disk5 shape: never decode it with the writable repository model.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrozenProjectV5 {
    project_id: String,
    name: String,
    repositories: Vec<String>,
    agent: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FrozenRegistryV5 {
    revision: u64,
    #[serde(deserialize_with = "objects")]
    projects: Vec<FrozenProjectV5>,
    #[serde(deserialize_with = "objects")]
    receipts: Vec<RetainedReceipt>,
    expired_ids: Vec<String>,
}
impl FrozenRegistryV5 {
    pub(super) fn migrate(self) -> Result<Registry> {
        for p in &self.projects {
            let mut urls = HashSet::new();
            if !valid_name(&p.name)
                || p.agent != "opencode"
                || p.repositories.len() > MAX_REPOSITORIES
                || !p
                    .repositories
                    .iter()
                    .all(|url| valid_locator(url) && urls.insert(url))
            {
                return Err("Invalid disk5 project metadata; preserved".into());
            }
        }
        validate_registry(
            self.revision,
            self.projects.iter().map(|p| p.project_id.as_str()),
            &self.receipts,
            &self.expired_ids,
        )?;
        let next = Registry {
            revision: self.revision,
            projects: self
                .projects
                .into_iter()
                .map(|p| Project {
                    key: format!("project-{}", p.project_id),
                    description: String::new(),
                    prompt: String::new(),
                    project_id: p.project_id,
                    name: p.name,
                    agent: p.agent,
                    repositories: p
                        .repositories
                        .into_iter()
                        .map(Repository::unknown)
                        .collect(),
                })
                .collect(),
            receipts: self.receipts,
            expired_ids: self.expired_ids,
        };
        next.validate()?;
        Ok(next)
    }
}

// Disk6–8 repositories require exact Work branches and have no read-only flag.
// Decode and validate this frozen shape before adding any disk9 metadata.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrozenRepositoryV6 {
    #[serde(deserialize_with = "required_nullable")]
    name: Option<String>,
    url: String,
    #[serde(deserialize_with = "object")]
    branch: Branch,
}
impl FrozenRepositoryV6 {
    fn migrate(self) -> Result<Repository> {
        if self
            .branch
            .work
            .as_deref()
            .is_some_and(|work| !valid_branch(work))
        {
            return Err("Invalid legacy project Work branch; preserved".into());
        }
        let next = Repository {
            name: self.name,
            url: self.url,
            read_only: true,
            branch: self.branch,
        };
        if !next.valid() {
            return Err("Invalid legacy project repository; preserved".into());
        }
        Ok(next)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrozenProjectV6 {
    project_id: String,
    name: String,
    #[serde(deserialize_with = "objects")]
    repositories: Vec<FrozenRepositoryV6>,
    agent: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FrozenRegistryV6 {
    revision: u64,
    #[serde(deserialize_with = "objects")]
    projects: Vec<FrozenProjectV6>,
    #[serde(deserialize_with = "objects")]
    receipts: Vec<RetainedReceipt>,
    expired_ids: Vec<String>,
}
impl FrozenRegistryV6 {
    pub(super) fn migrate(self) -> Result<Registry> {
        validate_registry(
            self.revision,
            self.projects.iter().map(|p| p.project_id.as_str()),
            &self.receipts,
            &self.expired_ids,
        )?;
        let next = Registry {
            revision: self.revision,
            projects: self
                .projects
                .into_iter()
                .map(|p| {
                    Ok(Project {
                        key: format!("project-{}", p.project_id),
                        description: String::new(),
                        prompt: String::new(),
                        project_id: p.project_id,
                        name: p.name,
                        repositories: p
                            .repositories
                            .into_iter()
                            .map(FrozenRepositoryV6::migrate)
                            .collect::<Result<_>>()?,
                        agent: p.agent,
                    })
                })
                .collect::<Result<_>>()?,
            receipts: self.receipts,
            expired_ids: self.expired_ids,
        };
        next.validate()?;
        Ok(next)
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Registry {
    revision: u64,
    #[serde(deserialize_with = "objects")]
    projects: Vec<Project>,
    #[serde(deserialize_with = "objects")]
    receipts: Vec<RetainedReceipt>,
    expired_ids: Vec<String>,
}
// Shared disk5/disk6 identity and replay invariants; validate frozen records
// before converting any repository metadata.
fn validate_registry<'a>(
    revision: u64,
    project_ids: impl ExactSizeIterator<Item = &'a str>,
    receipts: &[RetainedReceipt],
    expired_ids: &[String],
) -> Result<()> {
    let fail = || "Invalid project registry; preserved".to_owned();
    if revision > v2::MAX_REVISION
        || project_ids.len() > MAX_PROJECTS
        || receipts.len() > MAX_RECEIPTS
        || expired_ids.len() > MAX_TOMBSTONES
        || receipts.last().map_or(0, |r| r.receipt.revision) != revision
        || receipts.last().is_some_and(|r| r.fingerprint.is_none())
    {
        return Err(fail());
    }
    let mut projects = HashSet::new();
    for id in project_ids {
        if !valid_id(id) || !projects.insert(id) {
            return Err(fail());
        }
    }
    let mut ids = HashSet::new();
    for id in expired_ids {
        if !valid_id(id) || !ids.insert(id) {
            return Err(fail());
        }
    }
    let mut previous = 0;
    for retained in receipts {
        let r = &retained.receipt;
        let exists = projects.contains(r.project_id.as_str());
        if !valid_id(&r.request_id)
            || !ids.insert(&r.request_id)
            || !valid_id(&r.project_id)
            || r.revision <= previous
            || r.revision > revision
            || match &retained.fingerprint {
                Some(fp) => {
                    fp.len() != 64
                        || !fp
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                        || r.deleted == exists
                }
                // A deleted project's older receipts stay queryable, without
                // retaining any fingerprint derived from its former metadata.
                None => r.deleted || exists,
            }
        {
            return Err(fail());
        }
        previous = r.revision;
    }
    if revision == 0 && (!projects.is_empty() || !expired_ids.is_empty()) {
        return Err(fail());
    }
    Ok(())
}

impl Registry {
    pub(super) fn freeze_retained(
        &self,
        project_id: &str,
        expected_revision: u64,
        global: &str,
    ) -> AppResult<retained_jobs::FrozenProject> {
        if self.revision != expected_revision {
            return Err(v2::stale());
        }
        let p = self
            .projects
            .iter()
            .find(|p| p.project_id == project_id)
            .ok_or_else(invalid)?;
        Ok(retained_jobs::FrozenProject {
            project_id: p.project_id.clone(),
            project_key: p.key.clone(),
            context: format!("Global:\n{global}\nProject:\n{}", p.prompt),
            repositories: p
                .repositories
                .iter()
                .map(|r| retained_jobs::Repository {
                    url: r.url.clone(),
                    reference: r.branch.base.clone(),
                    read_only: r.read_only,
                })
                .collect(),
        })
    }
    pub(super) fn validate(&self) -> Result<()> {
        let mut keys = HashSet::new();
        if self
            .projects
            .iter()
            .any(|p| !p.valid() || !keys.insert(&p.key))
        {
            return Err("Invalid project metadata; preserved".into());
        }
        validate_registry(
            self.revision,
            self.projects.iter().map(|p| p.project_id.as_str()),
            &self.receipts,
            &self.expired_ids,
        )
    }
    fn retained(&self, id: &str, fingerprint: &str) -> AppResult<Option<Value>> {
        if let Some(r) = self.receipts.iter().find(|r| r.receipt.request_id == id) {
            let fp = r.fingerprint.as_ref().ok_or_else(expired)?;
            if fp != fingerprint {
                return Err(AppError::new(
                    "ID_CONFLICT",
                    "Project request identifier is bound to different input",
                ));
            }
            return Ok(Some(json!(r.receipt)));
        }
        if self.expired_ids.iter().any(|expired| expired == id) {
            return Err(expired());
        }
        Ok(None)
    }
    fn make_room(&mut self) -> AppResult<()> {
        if self.receipts.len() < MAX_RECEIPTS {
            return Ok(());
        }
        if self.expired_ids.len() >= MAX_TOMBSTONES {
            return Err(v2::capacity());
        }
        self.expired_ids
            .push(self.receipts.remove(0).receipt.request_id);
        Ok(())
    }
}
fn expired() -> AppError {
    AppError::new(
        "REQUEST_EXPIRED",
        "Expired project request cannot be reused",
    )
}
fn missing() -> AppError {
    AppError::new(
        "PROJECT_NOT_FOUND",
        "Project is absent; no replacement was created",
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    request_id: String,
    instance_id: String,
    expected_revision: u64,
    name: String,
    key: String,
    description: String,
    prompt: String,
    #[serde(deserialize_with = "objects")]
    repositories: Vec<Repository>,
    agent: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    request_id: String,
    instance_id: String,
    expected_revision: u64,
    project_id: String,
    name: String,
    key: String,
    description: String,
    prompt: String,
    #[serde(deserialize_with = "objects")]
    repositories: Vec<Repository>,
    agent: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Delete {
    request_id: String,
    instance_id: String,
    expected_revision: u64,
    project_id: String,
}

// Only the safe absent-state startup path may call this factory. Shared defaults
// remain empty for existing profiles and frozen legacy migrations.
pub(super) fn bootstrap(path: &std::path::Path) -> Result<Durable> {
    let request_id = new_id()?;
    let seed = new_id()?;
    let project_id = format!("{}{:016x}", &seed[..16], 1);
    let mut repository = Repository::unknown("https://github.com/jdharrison/smith".into());
    repository.name = Some("smith".into());
    repository.branch.base = Some("main".into());
    repository.branch.work = Some("main".into());
    let project = Project {
        project_id: project_id.clone(),
        name: "smith".into(),
        key: "smith".into(),
        description: "local autonomy subtrate".into(),
        prompt: String::new(),
        repositories: vec![repository],
        agent: "opencode".into(),
    };
    let fp = fingerprint("v3:projects:bootstrap", &json!(project))
        .map_err(|_| "Cannot fingerprint initial project")?;
    let mut durable = Durable::default();
    durable.revision = 1;
    durable.projects = Registry {
        revision: 1,
        projects: vec![project],
        receipts: vec![RetainedReceipt {
            receipt: Receipt {
                request_id,
                project_id,
                revision: 1,
                deleted: false,
            },
            fingerprint: Some(fp),
        }],
        expired_ids: vec![],
    };
    durable.validate()?;
    durable
        .preflight()
        .map_err(|_| "Initial application state exceeds capacity")?;
    let bytes = serde_json::to_vec(&durable).map_err(|_| "Cannot encode application state")?;
    if bytes.len() as u64 > STATE_LIMIT {
        return Err("Application state exceeds durable limit".into());
    }
    // Never replace/adopt a destination that appeared after the absent read.
    // A publication error may be uncertain; leave recovery to the operator.
    filesystem::publish(path, &bytes, 0o600, false)?;
    Ok(durable)
}

impl Application {
    pub(super) fn call_projects(&self, req: AppRequest) -> AppResult<Value> {
        if matches!(
            req.op.as_str(),
            "projects_list"
                | "project_create"
                | "project_update"
                | "project_delete"
                | "projects_list_v2"
                | "project_create_v2"
                | "project_update_v2"
                | "project_delete_v2"
        ) {
            return Err(AppError::new(
                "UPGRADE_REQUIRED",
                "Projects v3 operations are required",
            ));
        }
        if !req.args.is_object() {
            return Err(invalid());
        }
        if req.op == "projects_list_v3" {
            let _: Empty = decode(req.args)?;
            let inner = self.inner.lock().map_err(|_| storage())?;
            if inner.fault || !inner.owned {
                return Err(storage());
            }
            let registry = &inner.durable.projects;
            let result = json!({"revision":registry.revision,"projects":registry.projects});
            return if reply_fits(&result) {
                Ok(result)
            } else {
                Err(v2::capacity())
            };
        }
        if req.op == "project_receipt" {
            let a: IdArgs = decode(req.args)?;
            if !valid_id(&a.request_id) {
                return Err(invalid());
            }
            let inner = self.inner.lock().map_err(|_| storage())?;
            if inner.fault || !inner.owned {
                return Err(storage());
            }
            let registry = &inner.durable.projects;
            return registry
                .receipts
                .iter()
                .find(|r| r.receipt.request_id == a.request_id)
                .map(|r| json!(r.receipt))
                .ok_or_else(|| {
                    if registry.expired_ids.contains(&a.request_id) {
                        expired()
                    } else {
                        AppError::new(
                            "REQUEST_UNKNOWN",
                            "Project request is unknown; observe before any new action",
                        )
                    }
                });
        }
        let id = req
            .args
            .get("request_id")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        if !valid_id(id) {
            return Err(invalid());
        }
        let id = id.to_owned();
        let fp = fingerprint(&format!("v3:projects:{}", req.op), &req.args)?;
        let mut inner = self.inner.lock().map_err(|_| storage())?;
        if inner.fault || !inner.owned {
            return Err(storage());
        }
        // Retained identity wins over fresh instance/revision/input admission.
        if let Some(receipt) = inner.durable.projects.retained(&id, &fp)? {
            return Ok(receipt);
        }
        let (instance, expected, target, metadata) = match req.op.as_str() {
            "project_create_v3" => {
                let a: Create = decode(req.args)?;
                debug_assert_eq!(a.request_id, id);
                (
                    a.instance_id,
                    a.expected_revision,
                    None,
                    Some(Metadata {
                        name: a.name,
                        key: a.key,
                        description: a.description,
                        prompt: a.prompt,
                        repositories: a.repositories,
                        agent: a.agent,
                    }),
                )
            }
            "project_update_v3" => {
                let a: Update = decode(req.args)?;
                debug_assert_eq!(a.request_id, id);
                (
                    a.instance_id,
                    a.expected_revision,
                    Some(a.project_id),
                    Some(Metadata {
                        name: a.name,
                        key: a.key,
                        description: a.description,
                        prompt: a.prompt,
                        repositories: a.repositories,
                        agent: a.agent,
                    }),
                )
            }
            "project_delete_v3" => {
                let a: Delete = decode(req.args)?;
                debug_assert_eq!(a.request_id, id);
                (a.instance_id, a.expected_revision, Some(a.project_id), None)
            }
            _ => return Err(invalid()),
        };
        if !valid_id(&instance)
            || expected > v2::MAX_REVISION
            || target.as_ref().is_some_and(|id| !valid_id(id))
            || metadata.as_ref().is_some_and(|metadata| !metadata.valid())
        {
            return Err(invalid());
        }
        if instance != self.instance_id {
            return Err(AppError::new(
                "INSTANCE_MISMATCH",
                "Service instance changed; synchronize without resending",
            ));
        }
        if expected != inner.durable.projects.revision {
            return Err(v2::stale());
        }
        if let Some(metadata) = &metadata {
            if inner
                .durable
                .projects
                .projects
                .iter()
                .any(|p| Some(&p.project_id) != target.as_ref() && p.key == metadata.key)
            {
                return Err(AppError::new(
                    "INVALID_REQUEST",
                    "Project key is already in use",
                ));
            }
        }
        let mut next = inner.durable.clone();
        let registry = &mut next.projects;
        let revision = registry
            .revision
            .checked_add(1)
            .filter(|r| *r <= v2::MAX_REVISION)
            .ok_or_else(v2::capacity)?;
        registry.make_room()?;
        let deleted = metadata.is_none();
        let project_id = if let Some(target) = target {
            let index = registry
                .projects
                .iter()
                .position(|p| p.project_id == target)
                .ok_or_else(missing)?;
            if let Some(metadata) = metadata {
                // The hidden compatibility name is not renamed with the human key.
                if metadata.name != registry.projects[index].name {
                    return Err(AppError::new(
                        "INVALID_REQUEST",
                        "Preserve the existing project compatibility name",
                    ));
                }
                registry.projects[index] = metadata.project(target.clone());
            } else {
                registry.projects.remove(index);
                for retained in &mut registry.receipts {
                    if retained.receipt.project_id == target {
                        retained.fingerprint = None;
                    }
                }
            }
            target
        } else {
            if registry.projects.len() >= MAX_PROJECTS {
                return Err(v2::capacity());
            }
            let seed = new_id().map_err(|_| storage())?;
            // A monotonic suffix prevents deleted identity reuse even if entropy repeats.
            let project_id = format!("{}{:016x}", &seed[..16], revision);
            if registry.projects.iter().any(|p| p.project_id == project_id)
                || registry
                    .receipts
                    .iter()
                    .any(|r| r.receipt.project_id == project_id)
            {
                return Err(v2::capacity());
            }
            // Ordinary new forms supply name=key; copies retain their source name.
            let metadata = metadata.ok_or_else(invalid)?;
            registry.projects.push(metadata.project(project_id.clone()));
            project_id
        };
        let receipt = Receipt {
            request_id: id,
            project_id,
            revision,
            deleted,
        };
        let result = json!(receipt);
        registry.revision = revision;
        registry.receipts.push(RetainedReceipt {
            receipt,
            fingerprint: Some(fp),
        });
        self.commit(&mut inner, next)?;
        Ok(result)
    }
}
