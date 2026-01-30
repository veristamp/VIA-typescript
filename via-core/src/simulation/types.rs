use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct OTelLog {
    pub resourceLogs: Vec<ResourceLog>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct ResourceLog {
    pub resource: Resource,
    pub scopeLogs: Vec<ScopeLog>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Resource {
    pub attributes: Vec<KeyValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct ScopeLog {
    pub logRecords: Vec<LogRecord>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct LogRecord {
    pub timeUnixNano: String,
    pub traceId: String,
    pub spanId: String,
    pub severityNumber: u32,
    pub severityText: String,
    pub body: AnyValue,
    pub attributes: Vec<KeyValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeyValue {
    pub key: String,
    pub value: AnyValue,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
#[allow(non_snake_case)]
pub enum AnyValue {
    String { stringValue: String },
    Int { intValue: i64 },
    Bool { boolValue: bool },
    Double { doubleValue: f64 },
}

impl AnyValue {
    pub fn string(s: impl Into<String>) -> Self {
        AnyValue::String {
            stringValue: s.into(),
        }
    }
    pub fn int(i: i64) -> Self {
        AnyValue::Int { intValue: i }
    }
    pub fn double(d: f64) -> Self {
        AnyValue::Double { doubleValue: d }
    }
    pub fn bool(b: bool) -> Self {
        AnyValue::Bool { boolValue: b }
    }
}
