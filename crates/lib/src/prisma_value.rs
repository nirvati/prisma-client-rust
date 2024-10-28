use std::{str::FromStr, sync::Arc};

use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive};
use indexmap::IndexMap;
use query_core::{
    constants::custom_types::{self},
    response_ir::Item as PrismaItem,
};
use serde::{Serialize, Serializer};
use uuid::Uuid;

use crate::scalar_types;

/// A Rust-friendly version of Prisma's own PrismaValue.
///
/// Prisma's PrismaValue has serialization overrides that make it suitable for JSON serialization,
/// but they lose some type information (eg. Bytes are encoded as base64), and can be less efficient
/// (eg. float values are encoded as strings).
///
/// This implementation only has an override for `PrismaValue::Null`, which is serialized as `None`
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum PrismaValue {
    String(scalar_types::String),
    Boolean(scalar_types::Boolean),
    Enum(String),
    Int(scalar_types::Int),
    Uuid(Uuid),
    List(Vec<PrismaValue>),
    Json(scalar_types::Json),
    Object(Vec<(String, PrismaValue)>),
    #[serde(serialize_with = "serialize_null")]
    Null,
    DateTime(scalar_types::DateTime),
    Float(scalar_types::Float),
    // Special variant for distinguishing between Float and Decimal
    Decimal(scalar_types::Decimal),
    BigInt(scalar_types::BigInt),
    Bytes(scalar_types::Bytes),
}

/// A Rust-friendly version of Prisma's own Item.
/// Exists solely for nicer conversion of query results to our PrismaValue.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Item {
    Map(IndexMap<String, Item>),
    List(Vec<Item>),
    Value(PrismaValue),
    Json(serde_json::Value),
    RawJson(prisma_value::RawJson),
}

impl From<PrismaItem> for Item {
    fn from(item: PrismaItem) -> Self {
        match item {
            PrismaItem::Map(map) => {
                Item::Map(map.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
            PrismaItem::List(list) => Item::List(list.into_iter().map(|v| v.into()).collect()),
            PrismaItem::Value(scalar) => Item::Value(scalar.into()),
            PrismaItem::Json(json) => Item::Json(json),
            PrismaItem::RawJson(json) => Item::RawJson(json),
            PrismaItem::Ref(arc) => Arc::try_unwrap(arc)
                .unwrap_or_else(|arc| (*arc).to_owned())
                .into(),
        }
    }
}

fn serialize_null<S>(serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    Option::<()>::None.serialize(serializer)
}

impl From<query_structure::PrismaValue> for PrismaValue {
    fn from(value: query_structure::PrismaValue) -> Self {
        match value {
            query_structure::PrismaValue::String(value) => Self::String(value),
            query_structure::PrismaValue::Boolean(value) => Self::Boolean(value),
            query_structure::PrismaValue::Enum(value) => Self::Enum(value),
            query_structure::PrismaValue::Int(value) => Self::Int(value as i32),
            query_structure::PrismaValue::Uuid(value) => Self::Uuid(value),
            query_structure::PrismaValue::List(value) => {
                Self::List(value.into_iter().map(Into::into).collect())
            }
            query_structure::PrismaValue::Json(value) => {
                Self::Json(serde_json::from_str(&value).unwrap())
            }
            query_structure::PrismaValue::Object(mut value) => {
                let type_position = value.iter().position(|(k, _)| k == custom_types::TYPE);

                if let Some((_, query_structure::PrismaValue::String(typ))) =
                    type_position.map(|pos| value.swap_remove(pos))
                {
                    let (_, value) = value.swap_remove(
                        value
                            .iter()
                            .position(|(k, _)| k == custom_types::VALUE)
                            .unwrap(),
                    );

                    match (typ.as_str(), value) {
                        (custom_types::DATETIME, query_structure::PrismaValue::DateTime(dt)) => {
                            PrismaValue::DateTime(dt)
                        }
                        (custom_types::BIGINT, query_structure::PrismaValue::BigInt(i)) => {
                            PrismaValue::BigInt(i)
                        }
                        (custom_types::DECIMAL, query_structure::PrismaValue::String(s)) => {
                            PrismaValue::Decimal(BigDecimal::from_str(&s).unwrap())
                        }
                        (custom_types::BYTES, query_structure::PrismaValue::Bytes(b)) => {
                            PrismaValue::Bytes(b)
                        }
                        (custom_types::JSON, query_structure::PrismaValue::Json(j)) => {
                            PrismaValue::Json(serde_json::from_str(&j).unwrap())
                        }
                        _ => unreachable!("Incorrect PrismaValue for {typ}"),
                    }
                } else {
                    Self::Object(value.into_iter().map(|(k, v)| (k, v.into())).collect())
                }
            }
            query_structure::PrismaValue::Null => Self::Null,
            query_structure::PrismaValue::DateTime(value) => Self::DateTime(value),
            query_structure::PrismaValue::Float(value) => Self::Float(value.to_f64().unwrap()),
            query_structure::PrismaValue::BigInt(value) => Self::BigInt(value),
            query_structure::PrismaValue::Bytes(value) => Self::Bytes(value),
        }
    }
}

impl From<PrismaValue> for query_structure::PrismaValue {
    fn from(val: PrismaValue) -> Self {
        match val {
            PrismaValue::String(value) => Self::String(value),
            PrismaValue::Boolean(value) => Self::Boolean(value),
            PrismaValue::Enum(value) => Self::Enum(value),
            PrismaValue::Int(value) => Self::Int(value as i64),
            PrismaValue::Uuid(value) => Self::Uuid(value),
            PrismaValue::List(value) => Self::List(value.into_iter().map(Into::into).collect()),
            PrismaValue::Json(value) => Self::Json(serde_json::to_string(&value).unwrap()),
            PrismaValue::Object(value) => {
                Self::Object(value.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
            PrismaValue::Null => Self::Null,
            PrismaValue::DateTime(value) => Self::DateTime(value),
            PrismaValue::Decimal(value) => {
                Self::Float(bigdecimal::BigDecimal::from_str(&value.to_string()).unwrap())
            }
            PrismaValue::Float(value) => {
                Self::Float(bigdecimal::BigDecimal::from_f64(value).unwrap())
            }
            PrismaValue::BigInt(value) => Self::BigInt(value),
            PrismaValue::Bytes(value) => Self::Bytes(value),
        }
    }
}
