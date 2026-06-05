use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const SOURCE_MODULE: &str = "project_schema";

// ─── Schema types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PropertyDef {
    #[serde(rename = "type", default)]
    pub prop_type: String,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PageTypeDef {
    #[serde(default)]
    pub uses: Vec<String>,
    #[serde(rename = "allowedStatuses", default)]
    pub allowed_statuses: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct BlockTypeDef {
    #[serde(default)]
    pub markers: HashMap<String, String>,
    #[serde(default)]
    pub uses: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RelationDef {
    #[serde(default)]
    pub source: Vec<String>,
    #[serde(default)]
    pub target: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RelationLabels {
    #[serde(rename = "forwardLabel", default)]
    pub forward_label: String,
    #[serde(rename = "inverseLabel", default)]
    pub inverse_label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LogseqRenderer {
    #[serde(default)]
    pub relations: HashMap<String, RelationLabels>,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RendererConfig {
    #[serde(default)]
    pub logseq: LogseqRenderer,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectSchema {
    #[serde(rename = "schemaVersion", default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub properties: HashMap<String, PropertyDef>,
    /// Keys are status names; values are null or future metadata.
    #[serde(default)]
    pub statuses: HashMap<String, serde_yaml::Value>,
    #[serde(rename = "pageTypes", default)]
    pub page_types: HashMap<String, PageTypeDef>,
    #[serde(rename = "blockTypes", default)]
    pub block_types: HashMap<String, BlockTypeDef>,
    #[serde(default)]
    pub relations: HashMap<String, RelationDef>,
    #[serde(default)]
    pub renderers: RendererConfig,
}

fn default_schema_version() -> u32 {
    1
}

// ─── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SchemaError {
    NotFound { searched: Vec<String> },
    ParseError { detail: String },
    ValidationFailed { rule: String, detail: String },
    AliasCollision { alias_value: String, collides_with: String },
}

impl SchemaError {
    pub fn event_type(&self) -> &'static str {
        match self {
            SchemaError::NotFound { .. } => "SchemaNotFound",
            SchemaError::ParseError { .. } => "SchemaParseError",
            SchemaError::ValidationFailed { .. } => "SchemaValidationFailed",
            SchemaError::AliasCollision { .. } => "SchemaAliasCollisionDetected",
        }
    }

    pub fn to_payload(&self) -> JsonValue {
        match self {
            SchemaError::NotFound { searched } => json!({
                "failure_reason": "schema_not_found",
                "searched_locations": searched,
            }),
            SchemaError::ParseError { detail } => json!({
                "failure_reason": "schema_parse_error",
                "detail": detail,
            }),
            SchemaError::ValidationFailed { rule, detail } => json!({
                "failure_reason": "schema_validation_failed",
                "violated_rule": rule,
                "detail": detail,
            }),
            SchemaError::AliasCollision { alias_value, collides_with } => json!({
                "failure_reason": "alias_collision",
                "alias_value": alias_value,
                "collides_with": collides_with,
            }),
        }
    }

    pub fn message(&self) -> String {
        match self {
            SchemaError::NotFound { searched } => {
                format!("No vocabulary definition found. Searched: {}", searched.join(", "))
            }
            SchemaError::ParseError { detail } => {
                format!("Schema parse error: {}", detail)
            }
            SchemaError::ValidationFailed { rule, detail } => {
                format!("Schema validation failed ({}): {}", rule, detail)
            }
            SchemaError::AliasCollision { alias_value, collides_with } => {
                format!(
                    "Alias collision: '{}' collides with '{}'",
                    alias_value, collides_with
                )
            }
        }
    }
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

// ─── Loading ──────────────────────────────────────────────────────────────────

/// Parse a schema directly from a YAML string. Useful for testing and
/// programmatic schema construction without a filesystem.
pub fn load_schema_str(yaml: &str) -> Result<ProjectSchema, SchemaError> {
    parse_schema(yaml)
}

/// Load the active schema for `project_dir`.
///
/// Resolution order:
///   1. `<project_dir>/project-schema.yaml` (merged with base if `extends:` present)
///   2. `~/.lucidpm/default-schema.yaml`
///   3. `SchemaNotFound` if neither is accessible
pub fn load_schema(project_dir: &Path) -> Result<ProjectSchema, SchemaError> {
    let project_path = project_dir.join("project-schema.yaml");
    let default_path = home_dir().map(|h| h.join(".lucidpm").join("default-schema.yaml"));

    let default_yaml = match &default_path {
        Some(p) if p.exists() => Some(fs::read_to_string(p).map_err(|e| SchemaError::ParseError {
            detail: format!("Cannot read default schema: {}", e),
        })?),
        _ => None,
    };

    if !project_path.exists() {
        return match default_yaml {
            Some(yaml) => parse_schema(&yaml),
            None => Err(SchemaError::NotFound {
                searched: vec![
                    "project vocabulary (project-schema.yaml)".to_string(),
                    "shared default (~/.lucidpm/default-schema.yaml)".to_string(),
                ],
            }),
        };
    }

    let project_raw = fs::read_to_string(&project_path).map_err(|e| SchemaError::ParseError {
        detail: format!("Cannot read project schema: {}", e),
    })?;

    let extends_yaml = resolve_extends_content(&project_raw)?;
    let base = extends_yaml.as_deref().or(default_yaml.as_deref());

    match base {
        Some(base_yaml) => merge_and_parse(base_yaml, &project_raw),
        None => parse_schema(&project_raw),
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Read the `extends:` path from a YAML string and return its file contents.
fn resolve_extends_content(project_yaml: &str) -> Result<Option<String>, SchemaError> {
    let raw: serde_yaml::Value = serde_yaml::from_str(project_yaml)
        .map_err(|e| SchemaError::ParseError { detail: e.to_string() })?;

    let path_str = raw
        .as_mapping()
        .and_then(|m| m.get(&serde_yaml::Value::String("extends".to_string())))
        .and_then(|v| v.as_str())
        .map(expand_tilde);

    match path_str {
        None => Ok(None),
        Some(p) => {
            let path = PathBuf::from(&p);
            if !path.exists() {
                return Err(SchemaError::ParseError {
                    detail: format!("extends: path not found: {}", p),
                });
            }
            let content = fs::read_to_string(&path).map_err(|e| SchemaError::ParseError {
                detail: format!("Cannot read extends target '{}': {}", p, e),
            })?;
            Ok(Some(content))
        }
    }
}

fn expand_tilde(s: &str) -> String {
    if s.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return s.replacen('~', &home, 1);
        }
    }
    s.to_string()
}

fn merge_and_parse(base_yaml: &str, override_yaml: &str) -> Result<ProjectSchema, SchemaError> {
    let base_val: serde_yaml::Value = serde_yaml::from_str(base_yaml)
        .map_err(|e| SchemaError::ParseError { detail: format!("Base schema: {}", e) })?;

    let mut override_val: serde_yaml::Value = serde_yaml::from_str(override_yaml)
        .map_err(|e| SchemaError::ParseError { detail: e.to_string() })?;

    // Strip `extends:` key before merging so it doesn't land in the struct
    if let serde_yaml::Value::Mapping(ref mut m) = override_val {
        m.remove(&serde_yaml::Value::String("extends".to_string()));
    }

    let merged = merge_yaml(base_val, override_val);
    serde_yaml::from_value(merged).map_err(|e| SchemaError::ParseError { detail: e.to_string() })
}

fn parse_schema(yaml_str: &str) -> Result<ProjectSchema, SchemaError> {
    serde_yaml::from_str(yaml_str).map_err(|e| SchemaError::ParseError { detail: e.to_string() })
}

/// Merge two YAML values: maps merge recursively; lists and scalars replace.
fn merge_yaml(base: serde_yaml::Value, over: serde_yaml::Value) -> serde_yaml::Value {
    match (base, over) {
        (serde_yaml::Value::Mapping(mut base_map), serde_yaml::Value::Mapping(over_map)) => {
            for (k, v) in over_map {
                let merged = match base_map.get(&k).cloned() {
                    Some(base_v) if v.is_mapping() && base_v.is_mapping() => {
                        merge_yaml(base_v, v)
                    }
                    _ => v,
                };
                base_map.insert(k, merged);
            }
            serde_yaml::Value::Mapping(base_map)
        }
        (_, over) => over,
    }
}

// ─── Validation ───────────────────────────────────────────────────────────────

/// Validate structural rules. Returns the first violation found.
///
/// Collision detection is two-phase to ensure deterministic `collides_with` messages:
///   Phase 1 — register all canonical names (sorted alphabetically)
///   Phase 2 — check each type's aliases against the canonical registry
/// This guarantees aliases are always checked against pre-registered canonical names,
/// making the `collides_with` field unambiguous regardless of HashMap iteration order.
pub fn validate(schema: &ProjectSchema) -> Result<(), SchemaError> {
    // Phase 1: register all canonical page type names (sorted for determinism)
    let mut registry: HashMap<String, String> = HashMap::new();
    let mut page_type_entries: Vec<(&String, &PageTypeDef)> = schema.page_types.iter().collect();
    page_type_entries.sort_by_key(|(name, _)| name.as_str());

    for (type_name, _) in &page_type_entries {
        registry.insert(type_name.to_string(), format!("pageType '{}'", type_name));
    }

    // Register block type canonical names; detect collision with page types
    let mut block_type_names: Vec<&String> = schema.block_types.keys().collect();
    block_type_names.sort();
    for type_name in &block_type_names {
        if let Some(existing) = registry.get(type_name.as_str()) {
            return Err(SchemaError::AliasCollision {
                alias_value: type_name.to_string(),
                collides_with: existing.clone(),
            });
        }
        registry.insert(type_name.to_string(), format!("blockType '{}'", type_name));
    }

    // Phase 2: check each type's aliases against the canonical registry,
    // then register the alias to catch alias-to-alias collisions
    for (type_name, def) in &page_type_entries {
        let mut aliases = def.aliases.clone();
        aliases.sort();
        for alias in &aliases {
            if let Some(existing) = registry.get(alias.as_str()) {
                return Err(SchemaError::AliasCollision {
                    alias_value: alias.clone(),
                    collides_with: existing.clone(),
                });
            }
            registry.insert(alias.clone(), format!("alias of pageType '{}'", type_name));
        }
    }

    // Validate uses: entries reference defined properties
    for (type_name, def) in &page_type_entries {
        for prop in &def.uses {
            if !schema.properties.contains_key(prop.as_str()) {
                return Err(SchemaError::ValidationFailed {
                    rule: "undefined_property_ref".to_string(),
                    detail: format!(
                        "pageType '{}' uses undefined property '{}'",
                        type_name, prop
                    ),
                });
            }
        }
        for status in &def.allowed_statuses {
            if !schema.statuses.contains_key(status.as_str()) {
                return Err(SchemaError::ValidationFailed {
                    rule: "undefined_status_ref".to_string(),
                    detail: format!(
                        "pageType '{}' allowedStatuses references undefined status '{}'",
                        type_name, status
                    ),
                });
            }
        }
    }

    for (type_name, def) in &schema.block_types {
        for prop in &def.uses {
            if !schema.properties.contains_key(prop.as_str()) {
                return Err(SchemaError::ValidationFailed {
                    rule: "undefined_property_ref".to_string(),
                    detail: format!(
                        "blockType '{}' uses undefined property '{}'",
                        type_name, prop
                    ),
                });
            }
        }
    }

    // Validate renderer mappings reference defined relations / properties
    for rel_name in schema.renderers.logseq.relations.keys() {
        if !schema.relations.contains_key(rel_name.as_str()) {
            return Err(SchemaError::ValidationFailed {
                rule: "undefined_relation_ref".to_string(),
                detail: format!(
                    "renderers.logseq.relations references undefined relation '{}'",
                    rel_name
                ),
            });
        }
    }

    for prop_name in schema.renderers.logseq.properties.keys() {
        if !schema.properties.contains_key(prop_name.as_str()) {
            return Err(SchemaError::ValidationFailed {
                rule: "undefined_property_ref".to_string(),
                detail: format!(
                    "renderers.logseq.properties references undefined property '{}'",
                    prop_name
                ),
            });
        }
    }

    Ok(())
}

// ─── Schema query helpers ─────────────────────────────────────────────────────

/// Resolve `type_name` to its canonical name, following aliases.
/// Returns `None` if the type is not in the schema.
pub fn resolve_type<'a>(schema: &'a ProjectSchema, type_name: &'a str) -> Option<&'a str> {
    if schema.page_types.contains_key(type_name) || schema.block_types.contains_key(type_name) {
        return Some(type_name);
    }
    for (canonical, def) in &schema.page_types {
        if def.aliases.iter().any(|a| a == type_name) {
            return Some(canonical.as_str());
        }
    }
    None
}

/// Check if `status` is valid for `type_name` according to the schema.
pub fn is_valid_status(schema: &ProjectSchema, type_name: &str, status: &str) -> bool {
    if let Some(def) = schema.page_types.get(type_name) {
        if !def.allowed_statuses.is_empty() {
            return def.allowed_statuses.iter().any(|s| s == status);
        }
    }
    schema.statuses.contains_key(status)
}

/// Map a Logseq task marker to its normalized domain status.
pub fn marker_to_status<'a>(schema: &'a ProjectSchema, marker: &str) -> Option<&'a str> {
    for def in schema.block_types.values() {
        if let Some(status) = def.markers.get(marker) {
            return Some(status.as_str());
        }
    }
    None
}

/// Forward label for a relation in the Logseq renderer.
pub fn logseq_forward_label<'a>(schema: &'a ProjectSchema, relation: &'a str) -> &'a str {
    schema
        .renderers
        .logseq
        .relations
        .get(relation)
        .map(|l| l.forward_label.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(relation)
}

/// Inverse label for a relation in the Logseq renderer.
pub fn logseq_inverse_label<'a>(schema: &'a ProjectSchema, relation: &'a str) -> &'a str {
    schema
        .renderers
        .logseq
        .relations
        .get(relation)
        .map(|l| l.inverse_label.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(relation)
}

/// Logseq property key for a domain property name.
pub fn logseq_property_key<'a>(schema: &'a ProjectSchema, property: &'a str) -> &'a str {
    schema
        .renderers
        .logseq
        .properties
        .get(property)
        .map(|s| s.as_str())
        .unwrap_or(property)
}

/// All status names defined in the schema.
pub fn all_status_names(schema: &ProjectSchema) -> Vec<&str> {
    schema.statuses.keys().map(|s| s.as_str()).collect()
}

/// Check if `type_name` resolves to a block type concept.
/// Representation Ban: concept resolution via resolve_type before the check.
pub fn is_block_type(schema: &ProjectSchema, type_name: &str) -> bool {
    resolve_type(schema, type_name)
        .map(|canonical| schema.block_types.contains_key(canonical))
        .unwrap_or(false)
}

/// Return the first block type that has a non-empty marker mapping (sorted
/// alphabetically for determinism), or None if no such block type exists.
/// The returned canonical name is the task block type concept for this vocabulary.
pub fn canonical_task_block_type<'a>(
    schema: &'a ProjectSchema,
) -> Option<(&'a str, &'a HashMap<String, String>)> {
    let mut entries: Vec<_> = schema.block_types.iter()
        .filter(|(_, def)| !def.markers.is_empty())
        .collect();
    entries.sort_by_key(|(name, _)| name.as_str());
    entries.into_iter().next().map(|(name, def)| (name.as_str(), &def.markers))
}

// ─── Event emission ───────────────────────────────────────────────────────────

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn emit_event(events_file: &Path, event_type: &str, correlation_id: &str, payload: JsonValue) {
    let event = json!({
        "event_id":       Uuid::new_v4().to_string(),
        "event_type":     event_type,
        "timestamp":      timestamp_ms(),
        "correlation_id": correlation_id,
        "source_module":  SOURCE_MODULE,
        "payload":        payload,
    });
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(events_file) {
        let _ = writeln!(file, "{}", event);
    }
}

/// Emit the appropriate FAILURE event for a `SchemaError`.
pub fn emit_schema_failure(events_file: &Path, error: &SchemaError, correlation_id: &str) {
    emit_event(events_file, error.event_type(), correlation_id, error.to_payload());
}

/// Emit `SchemaTypeUnknown` (OBSERVATIONAL) when an item type is not in the schema.
pub fn emit_type_unknown(
    events_file: &Path,
    item_id: &str,
    unknown_type: &str,
    correlation_id: &str,
) {
    emit_event(
        events_file,
        "SchemaTypeUnknown",
        correlation_id,
        json!({
            "item_id":      item_id,
            "unknown_type": unknown_type,
        }),
    );
}

/// Convenience: load + validate, emitting failure events and printing to stderr on error.
/// Returns `Some(schema)` on success, `None` on any failure.
pub fn load_and_validate(
    project_dir: &Path,
    events_file: &Path,
    correlation_id: &str,
) -> Option<ProjectSchema> {
    match load_schema(project_dir) {
        Err(e) => {
            emit_schema_failure(events_file, &e, correlation_id);
            eprintln!("error: {}", e);
            None
        }
        Ok(schema) => match validate(&schema) {
            Err(e) => {
                emit_schema_failure(events_file, &e, correlation_id);
                eprintln!("error: {}", e);
                None
            }
            Ok(()) => Some(schema),
        },
    }
}
